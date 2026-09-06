use std::sync::Arc;

use anyhow::Result;

use crate::challenge::Challenge;
use crate::llm::LlmClient;

// ── Strategy types ──────────────────────────────

#[derive(Debug, Clone)]
pub enum Strategy {
    Fresh,
    Evolve {
        parent_source: String,
        parent_score: f64,
    },
    Analyze {
        sources_and_scores: Vec<(String, f64)>,
    },
    Crossover {
        source_a: String,
        source_b: String,
    },
}

impl Strategy {
    pub fn name(&self) -> &str {
        match self {
            Self::Fresh => "fresh",
            Self::Evolve { .. } => "evolve",
            Self::Analyze { .. } => "analyze",
            Self::Crossover { .. } => "crossover",
        }
    }
}

// ── Candidate generation ────────────────────────

pub async fn generate_candidates(
    llm: Arc<dyn LlmClient>,
    challenge: &Challenge,
    strategy: &Strategy,
    count: u32,
) -> Result<Vec<String>> {
    let (system, user) = build_prompts(challenge, strategy);
    let func_name = crate::challenge::extract_func_name(&challenge.signature);

    let mut handles = Vec::new();
    for _ in 0..count {
        let llm = llm.clone();
        let system = system.clone();
        let user = user.clone();
        let func_name = func_name.clone();
        handles.push(tokio::spawn(async move {
            let response = llm.prompt(&system, &user).await?;
            Ok::<_, anyhow::Error>(extract_rust_code(&response, &func_name))
        }));
    }

    let mut candidates = Vec::new();
    for handle in handles {
        match handle.await {
            Ok(Ok(Some(code))) => candidates.push(code),
            Ok(Ok(None)) => {} // couldn't extract valid code
            Ok(Err(e)) => {
                crate::ui::status(&format!("LLM error: {e}"));
            }
            Err(e) => {
                crate::ui::status(&format!("task join error: {e}"));
            }
        }
    }

    Ok(candidates)
}

// ── Prompt construction ─────────────────────────

fn build_prompts(challenge: &Challenge, strategy: &Strategy) -> (String, String) {
    match strategy {
        Strategy::Fresh => {
            let system = "You are a Rust optimization expert. Write the most performant \
                          implementation of the given function. Output ONLY the function \
                          implementation inside a ```rust code block. No explanation."
                .to_string();
            let user = format!(
                "Implement this function as optimally as possible:\n\n\
                 Signature: {sig}\n\
                 Description: {desc}\n\n\
                 Requirements:\n\
                 - Use only stdlib (no external crates)\n\
                 - Must match the exact signature\n\
                 - Focus on: minimal allocations, SIMD-friendly patterns, cache locality",
                sig = challenge.signature,
                desc = challenge.description,
            );
            (system, user)
        }
        Strategy::Evolve {
            parent_source,
            parent_score,
        } => {
            let system = "You are a Rust optimization expert. You will be given a function \
                          implementation and its performance score. Improve it to achieve a \
                          higher score. Output ONLY the improved function inside a ```rust \
                          code block."
                .to_string();
            let user = format!(
                "Current implementation (score: {parent_score:.3}):\n\
                 ```rust\n{parent_source}\n```\n\n\
                 Signature: {sig}\n\
                 Description: {desc}\n\n\
                 Improve this implementation. Focus on reducing time and memory usage.",
                sig = challenge.signature,
                desc = challenge.description,
            );
            (system, user)
        }
        Strategy::Analyze { sources_and_scores } => {
            let system = "You are a Rust optimization expert. Analyze the given implementations \
                          and write a new, better one combining the best techniques. Output ONLY \
                          the function inside a ```rust code block."
                .to_string();
            let mut impls = String::new();
            for (i, (src, score)) in sources_and_scores.iter().enumerate() {
                impls.push_str(&format!(
                    "\nImplementation {n} (score: {score:.3}):\n```rust\n{src}\n```\n",
                    n = i + 1,
                ));
            }
            let user = format!(
                "Here are several implementations with their scores:\n{impls}\n\
                 Signature: {sig}\n\
                 Write a new implementation that combines the best aspects.",
                sig = challenge.signature,
            );
            (system, user)
        }
        Strategy::Crossover { source_a, source_b } => {
            let system = "You are a Rust optimization expert. Combine the best aspects of two \
                          implementations into one. Output ONLY the function inside a ```rust \
                          code block."
                .to_string();
            let user = format!(
                "Implementation A:\n```rust\n{source_a}\n```\n\n\
                 Implementation B:\n```rust\n{source_b}\n```\n\n\
                 Signature: {sig}\n\
                 Combine the best optimization techniques from both.",
                sig = challenge.signature,
            );
            (system, user)
        }
    }
}

// ── Code extraction ─────────────────────────────

/// Extract a Rust function from an LLM response, stripping markdown fences.
pub fn extract_rust_code(response: &str, func_name: &str) -> Option<String> {
    let target = format!("fn {func_name}");

    // Try ```rust ... ```
    if let Some(code) = extract_fenced_block(response, "```rust") {
        if code.contains(&target) {
            return Some(code);
        }
    }

    // Try ``` ... ```
    if let Some(code) = extract_fenced_block(response, "```") {
        if code.contains(&target) {
            return Some(code);
        }
    }

    // Try raw (no fences)
    if response.contains(&target) {
        return Some(response.trim().to_string());
    }

    None
}

fn extract_fenced_block(text: &str, fence_start: &str) -> Option<String> {
    let start = text.find(fence_start)?;
    let after_fence = start + fence_start.len();

    // Skip to next newline (past language identifier)
    let code_start = after_fence + text[after_fence..].find('\n')? + 1;

    let code_end = code_start + text[code_start..].find("```")?;
    Some(text[code_start..code_end].trim().to_string())
}

// ── Strategy selection ──────────────────────────

pub async fn select_strategy(
    generation: u32,
    pool: &sqlx::SqlitePool,
    challenge_id: &str,
) -> Result<Strategy> {
    if generation == 1 {
        return Ok(Strategy::Fresh);
    }

    let best = crate::db::get_best_candidates(pool, challenge_id, 5).await?;
    if best.is_empty() {
        return Ok(Strategy::Fresh);
    }

    let cycle = (generation - 2) % 4;
    Ok(match cycle {
        0 => Strategy::Evolve {
            parent_source: best[0].source_code.clone(),
            parent_score: best[0].score,
        },
        1 => Strategy::Fresh,
        2 => Strategy::Analyze {
            sources_and_scores: best
                .iter()
                .map(|c| (c.source_code.clone(), c.score))
                .collect(),
        },
        3 => {
            if best.len() >= 2 {
                Strategy::Crossover {
                    source_a: best[0].source_code.clone(),
                    source_b: best[1].source_code.clone(),
                }
            } else {
                Strategy::Fresh
            }
        }
        _ => unreachable!(),
    })
}
