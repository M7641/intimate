//! oracle — a Rust agent whose sole job is to collect reading material and get
//! better at it from your feedback.
//!
//! Pipeline: **ingest** (broad, cheap) → **curate** (the product: a fast
//! embedding loop + a slow LLM-judged loop) → **serve** (Axum + SSE).
//!
//! lib.rs/main.rs split so integration tests can reference `oracle::` types.

pub mod config;
pub mod curate;
pub mod db;
pub mod embed;
pub mod ingest;
pub mod llm;
pub mod models;
pub mod profile;
pub mod server;
