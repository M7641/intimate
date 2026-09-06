use anyhow::Result;
use sqlx::sqlite::SqlitePoolOptions;
use sqlx::SqlitePool;

pub async fn init_pool(db_path: &str) -> Result<SqlitePool> {
    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect(&format!("sqlite:{db_path}?mode=rwc"))
        .await?;

    sqlx::query("PRAGMA journal_mode=WAL")
        .execute(&pool)
        .await?;

    sqlx::migrate!("./migrations").run(&pool).await?;

    Ok(pool)
}

// ── Challenge CRUD ──────────────────────────────

pub async fn upsert_challenge(
    pool: &SqlitePool,
    challenge: &crate::challenge::Challenge,
) -> Result<String> {
    let existing: Option<(String,)> = sqlx::query_as("SELECT id FROM challenges WHERE name = ?")
        .bind(&challenge.name)
        .fetch_optional(pool)
        .await?;

    if let Some((id,)) = existing {
        return Ok(id);
    }

    let id = uuid::Uuid::new_v4().to_string();
    let now = now_rfc3339();

    sqlx::query(
        "INSERT INTO challenges (id, name, description, signature, created_at)
         VALUES (?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(&challenge.name)
    .bind(&challenge.description)
    .bind(&challenge.signature)
    .bind(&now)
    .execute(pool)
    .await?;

    // Insert test cases
    for test in &challenge.tests {
        let test_id = uuid::Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO test_cases (id, challenge_id, input, expected) VALUES (?, ?, ?, ?)",
        )
        .bind(&test_id)
        .bind(&id)
        .bind(&test.input)
        .bind(&test.expected)
        .execute(pool)
        .await?;
    }

    Ok(id)
}

// ── Candidate CRUD ──────────────────────────────

pub async fn insert_candidate(
    pool: &SqlitePool,
    challenge_id: &str,
    generation: u32,
    strategy: &str,
    source_code: &str,
) -> Result<String> {
    let id = uuid::Uuid::new_v4().to_string();
    let now = now_rfc3339();

    sqlx::query(
        "INSERT INTO candidates (id, challenge_id, generation, strategy, source_code, created_at)
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(challenge_id)
    .bind(generation)
    .bind(strategy)
    .bind(source_code)
    .bind(&now)
    .execute(pool)
    .await?;

    Ok(id)
}

// ── Result CRUD ─────────────────────────────────

pub async fn insert_result(
    pool: &SqlitePool,
    candidate_id: &str,
    result: &crate::harness::HarnessResult,
    score: f64,
) -> Result<()> {
    let id = uuid::Uuid::new_v4().to_string();

    sqlx::query(
        "INSERT INTO results (id, candidate_id, passed, score, time_ns, memory_bytes, binary_size, compile_time_ns, error)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(candidate_id)
    .bind(result.passed as i32)
    .bind(score)
    .bind(result.time_ns as i64)
    .bind(result.memory_bytes as i64)
    .bind(result.binary_size as i64)
    .bind(result.compile_time_ns as i64)
    .bind(&result.error)
    .execute(pool)
    .await?;

    Ok(())
}

// ── Queries ─────────────────────────────────────

#[derive(Debug, Clone)]
pub struct CandidateRow {
    pub id: String,
    pub generation: i32,
    pub strategy: String,
    pub source_code: String,
    pub score: f64,
    pub time_ns: i64,
    pub memory_bytes: i64,
}

pub async fn get_best_candidates(
    pool: &SqlitePool,
    challenge_id: &str,
    limit: u32,
) -> Result<Vec<CandidateRow>> {
    let rows: Vec<(String, i32, String, String, f64, i64, i64)> = sqlx::query_as(
        "SELECT c.id, c.generation, c.strategy, c.source_code, r.score, r.time_ns, r.memory_bytes
         FROM candidates c
         JOIN results r ON r.candidate_id = c.id
         WHERE c.challenge_id = ? AND r.passed = 1
         ORDER BY r.score DESC
         LIMIT ?",
    )
    .bind(challenge_id)
    .bind(limit)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(
            |(id, generation, strategy, source_code, score, time_ns, memory_bytes)| CandidateRow {
                id,
                generation,
                strategy,
                source_code,
                score,
                time_ns,
                memory_bytes,
            },
        )
        .collect())
}

#[derive(Debug, Clone)]
pub struct ChallengeSummary {
    pub name: String,
    pub best_score: f64,
    pub total_candidates: i64,
}

pub async fn list_challenges_with_scores(pool: &SqlitePool) -> Result<Vec<ChallengeSummary>> {
    let rows: Vec<(String, f64, i64)> = sqlx::query_as(
        "SELECT ch.name,
                COALESCE(MAX(r.score), 0.0),
                COUNT(c.id)
         FROM challenges ch
         LEFT JOIN candidates c ON c.challenge_id = ch.id
         LEFT JOIN results r ON r.candidate_id = c.id
         GROUP BY ch.id
         ORDER BY ch.name",
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|(name, best_score, total_candidates)| ChallengeSummary {
            name,
            best_score,
            total_candidates,
        })
        .collect())
}

#[derive(Debug, Clone)]
pub struct LeaderboardRow {
    pub rank: usize,
    pub generation: i32,
    pub strategy: String,
    pub score: f64,
    pub time_ns: i64,
    pub memory_bytes: i64,
    pub binary_size: i64,
}

pub async fn get_leaderboard(
    pool: &SqlitePool,
    challenge_name: &str,
) -> Result<Vec<LeaderboardRow>> {
    let rows: Vec<(i32, String, f64, i64, i64, i64)> = sqlx::query_as(
        "SELECT c.generation, c.strategy, r.score, r.time_ns, r.memory_bytes, r.binary_size
         FROM candidates c
         JOIN results r ON r.candidate_id = c.id
         JOIN challenges ch ON ch.id = c.challenge_id
         WHERE ch.name = ? AND r.passed = 1
         ORDER BY r.score DESC",
    )
    .bind(challenge_name)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .enumerate()
        .map(
            |(i, (generation, strategy, score, time_ns, memory_bytes, binary_size))| {
                LeaderboardRow {
                    rank: i + 1,
                    generation,
                    strategy,
                    score,
                    time_ns,
                    memory_bytes,
                    binary_size,
                }
            },
        )
        .collect())
}

fn now_rfc3339() -> String {
    // Simple UTC timestamp without chrono dependency
    let dur = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = dur.as_secs();
    // Format as ISO 8601 (approximate, good enough for sorting)
    let days = secs / 86400;
    let remaining = secs % 86400;
    let hours = remaining / 3600;
    let minutes = (remaining % 3600) / 60;
    let seconds = remaining % 60;

    // Days since epoch to date (simplified)
    let mut y = 1970i64;
    let mut d = days as i64;
    loop {
        let days_in_year = if y % 4 == 0 && (y % 100 != 0 || y % 400 == 0) {
            366
        } else {
            365
        };
        if d < days_in_year {
            break;
        }
        d -= days_in_year;
        y += 1;
    }
    let leap = y % 4 == 0 && (y % 100 != 0 || y % 400 == 0);
    let month_days = [
        31,
        if leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    let mut m = 0;
    for &md in &month_days {
        if d < md {
            break;
        }
        d -= md;
        m += 1;
    }
    format!(
        "{y:04}-{:02}-{:02}T{hours:02}:{minutes:02}:{seconds:02}Z",
        m + 1,
        d + 1,
    )
}
