//! Configuration, sourced from the environment with sane defaults.
//!
//! Everything is tunable without a rebuild — the thresholds below are the knobs
//! that shape how aggressive the two curation lanes are.

#[derive(Debug, Clone)]
pub struct Config {
    /// Address the HTTP/SSE server binds to.
    pub bind: String,
    /// SQLite database path.
    pub db_path: String,
    /// Dimensionality of the lexical embedder's vectors.
    pub embed_dim: usize,
    /// How often (seconds) the background digest runs over the pending pool.
    pub digest_interval_secs: u64,
    /// Max items the digest sends to the LLM judge per batch (cost ceiling).
    pub digest_shortlist: usize,
    /// Live lane: minimum learned relevance to stream an item immediately.
    pub live_threshold: f32,
    /// Live lane: skip if an item is this similar to a recently-streamed one.
    pub dup_threshold: f32,
}

impl Config {
    pub fn from_env() -> Self {
        Self {
            bind: env("ORACLE_BIND", "127.0.0.1:7878"),
            db_path: env("ORACLE_DB", "oracle.db"),
            embed_dim: env_parse("ORACLE_EMBED_DIM", 256),
            digest_interval_secs: env_parse("ORACLE_DIGEST_INTERVAL", 120),
            digest_shortlist: env_parse("ORACLE_DIGEST_SHORTLIST", 20),
            live_threshold: env_parse("ORACLE_LIVE_THRESHOLD", 0.35),
            dup_threshold: env_parse("ORACLE_DUP_THRESHOLD", 0.92),
        }
    }
}

fn env(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_string())
}

fn env_parse<T: std::str::FromStr>(key: &str, default: T) -> T {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}
