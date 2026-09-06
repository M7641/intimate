use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::Result;
use sqlx::SqlitePool;
use tokio::sync::mpsc::UnboundedSender;
use tokio::sync::Semaphore;
use tokio::task::JoinSet;

use crate::llm::LlmClient;

// ── Events ──────────────────────────────────────

#[derive(Debug, Clone)]
pub enum GymEvent {
    Started {
        challenge_name: String,
        max_generations: u32,
    },
    ReferenceCompiled {
        time_ns: u64,
        memory_bytes: u64,
        binary_size: u64,
    },
    GenerationBegin {
        generation: u32,
    },
    CandidateGenerated {
        generation: u32,
        index: u32,
        strategy: String,
    },
    CandidateEvaluated {
        generation: u32,
        index: u32,
        passed: bool,
        score: f64,
        time_ns: u64,
        memory_bytes: u64,
    },
    GenerationEnd {
        generation: u32,
        best_score: f64,
    },
    Finished {
        best_score: f64,
        total_candidates: u32,
        elapsed_secs: f64,
    },
    Log {
        message: String,
    },
}

fn emit(tx: &UnboundedSender<GymEvent>, event: GymEvent) {
    let _ = tx.send(event);
}

// ── Main training loop ──────────────────────────

pub async fn run(
    challenge_path: PathBuf,
    llm: Arc<dyn LlmClient>,
    pool: SqlitePool,
    tx: UnboundedSender<GymEvent>,
) -> Result<()> {
    let start_time = Instant::now();

    // 1. Load challenge
    let loaded = crate::challenge::load(&challenge_path)?;
    let config = loaded.challenge.config.clone();

    emit(
        &tx,
        GymEvent::Started {
            challenge_name: loaded.challenge.name.clone(),
            max_generations: config.max_generations,
        },
    );

    // 2. Compile and benchmark reference
    emit(
        &tx,
        GymEvent::Log {
            message: "compiling reference implementation...".into(),
        },
    );

    let ref_result = crate::harness::evaluate_candidate(
        &loaded.reference_source,
        &loaded.challenge.tests,
        Duration::from_secs(config.timeout_compile_secs),
        Duration::from_secs(config.timeout_run_secs),
    )
    .await?;

    if !ref_result.passed {
        anyhow::bail!(
            "reference implementation failed tests: {}",
            ref_result.error
        );
    }

    emit(
        &tx,
        GymEvent::ReferenceCompiled {
            time_ns: ref_result.time_ns,
            memory_bytes: ref_result.memory_bytes,
            binary_size: ref_result.binary_size,
        },
    );

    // 3. Upsert challenge in DB
    let challenge_id = crate::db::upsert_challenge(&pool, &loaded.challenge).await?;
    let ref_result = Arc::new(ref_result);

    // 4. Training loop
    let mut total_candidates: u32 = 0;
    let mut global_best_score: f64 = 0.0;

    for gen in 1..=config.max_generations {
        emit(&tx, GymEvent::GenerationBegin { generation: gen });

        // Select strategy
        let strategy = crate::generator::select_strategy(gen, &pool, &challenge_id).await?;

        emit(
            &tx,
            GymEvent::Log {
                message: format!("gen {gen}: strategy = {}", strategy.name()),
            },
        );

        // Generate candidates via LLM
        let candidates = crate::generator::generate_candidates(
            llm.clone(),
            &loaded.challenge,
            &strategy,
            config.candidates_per_generation,
        )
        .await?;

        if candidates.is_empty() {
            emit(
                &tx,
                GymEvent::Log {
                    message: format!("gen {gen}: no valid candidates generated"),
                },
            );
            continue;
        }

        // Evaluate candidates in parallel (bounded concurrency)
        let semaphore = Arc::new(Semaphore::new(4));
        let mut join_set = JoinSet::new();

        for (i, source) in candidates.into_iter().enumerate() {
            let sem = semaphore.clone();
            let tests = loaded.challenge.tests.clone();
            let config = config.clone();
            let tx = tx.clone();
            let pool = pool.clone();
            let challenge_id = challenge_id.clone();
            let strategy_name = strategy.name().to_string();
            let ref_result = ref_result.clone();

            join_set.spawn(async move {
                let _permit = sem.acquire().await.unwrap();

                emit(
                    &tx,
                    GymEvent::CandidateGenerated {
                        generation: gen,
                        index: i as u32,
                        strategy: strategy_name.clone(),
                    },
                );

                let result = crate::harness::evaluate_candidate(
                    &source,
                    &tests,
                    Duration::from_secs(config.timeout_compile_secs),
                    Duration::from_secs(config.timeout_run_secs),
                )
                .await?;

                let score = crate::scorer::score(&result, &ref_result);

                // Store in DB
                let candidate_id =
                    crate::db::insert_candidate(&pool, &challenge_id, gen, &strategy_name, &source)
                        .await?;
                crate::db::insert_result(&pool, &candidate_id, &result, score).await?;

                emit(
                    &tx,
                    GymEvent::CandidateEvaluated {
                        generation: gen,
                        index: i as u32,
                        passed: result.passed,
                        score,
                        time_ns: result.time_ns,
                        memory_bytes: result.memory_bytes,
                    },
                );

                Ok::<_, anyhow::Error>(score)
            });
        }

        let mut gen_best_score: f64 = 0.0;
        let mut gen_count: u32 = 0;
        while let Some(result) = join_set.join_next().await {
            gen_count += 1;
            if let Ok(Ok(score)) = result {
                gen_best_score = gen_best_score.max(score);
            }
        }

        total_candidates += gen_count;
        global_best_score = global_best_score.max(gen_best_score);

        emit(
            &tx,
            GymEvent::GenerationEnd {
                generation: gen,
                best_score: gen_best_score,
            },
        );
    }

    emit(
        &tx,
        GymEvent::Finished {
            best_score: global_best_score,
            total_candidates,
            elapsed_secs: start_time.elapsed().as_secs_f64(),
        },
    );

    Ok(())
}
