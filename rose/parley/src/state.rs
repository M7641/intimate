//! Shared application state and provider selection.
//!
//! Which provider is real vs stub is decided here from the environment: if a
//! whisper model path is set, use whisper; otherwise fall back to the stub. This
//! is what lets the app run end-to-end before every model is installed.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use uuid::Uuid;

use crate::domain::{Language, Level, Turn};
use crate::memory::fs::FsMemoryStore;
use crate::memory::s3::{S3Config, S3MemoryStore};
use crate::memory::MemoryStore;
use crate::pedagogy;
use crate::providers::{
    anthropic::Anthropic, candle_llm::CandleLlm, candle_whisper::CandleWhisper, piper::Piper, stub,
    wiktionary::WiktionaryDictionary, Conversant, Dictionary, Synthesizer, Transcriber,
};

/// Per-conversation history, keyed by a client-supplied session id.
type Sessions = Mutex<HashMap<Uuid, Vec<Turn>>>;

#[derive(Clone)]
pub struct AppState {
    pub transcriber: Arc<dyn Transcriber>,
    pub conversant: Arc<dyn Conversant>,
    pub synthesizer: Arc<dyn Synthesizer>,
    /// Word → definition, from a trusted dictionary (the Wiktionnaire by default).
    pub dictionary: Arc<dyn Dictionary>,
    /// Durable, cross-session memories about the learner — the learner-state spine.
    pub memory: Arc<dyn MemoryStore>,
    pub sessions: Arc<Sessions>,
    pub language: Language,
    pub level: Level,
}

impl AppState {
    /// Build state from environment variables, choosing real providers where they
    /// are configured and stubs otherwise. Logs each choice so it is obvious at
    /// startup what is live.
    pub fn from_env() -> Self {
        let language = Language::French;
        let level = Level::Beginner;

        // LLM (the conversational brain). Selection order:
        //   1. Anthropic Claude, if ANTHROPIC_API_TOKEN_WORK is set (cloud, no
        //      weights to download — fastest way to a strong tutor).
        //   2. A quantized model loaded in-process via Candle otherwise; weights
        //      are pulled from the HF Hub on first run.
        //   3. The stub if Candle also fails to load (cold cache, bad file name).
        // Every path logs which one won so it is obvious at startup.
        let conversant: Arc<dyn Conversant> = match std::env::var("ANTHROPIC_API_TOKEN_WORK") {
            Ok(key) if !key.is_empty() => {
                let model = Anthropic::default_model();
                tracing::info!(%model, "conversant: anthropic (cloud)");
                Arc::new(Anthropic::new(key, model))
            }
            _ => {
                let repo = env_or("PARLEY_LLM_REPO", "Qwen/Qwen2.5-3B-Instruct-GGUF");
                let file = env_or("PARLEY_LLM_FILE", "qwen2.5-3b-instruct-q4_k_m.gguf");
                let tokenizer = env_or("PARLEY_LLM_TOKENIZER", "Qwen/Qwen2.5-3B-Instruct");
                match CandleLlm::load(&repo, &file, &tokenizer) {
                    Ok(llm) => {
                        tracing::info!(%repo, %file, "conversant: candle (in-process)");
                        Arc::new(llm)
                    }
                    Err(e) => {
                        tracing::error!(error = %e, "conversant: STUB (candle load failed)");
                        Arc::new(stub::StubConversant)
                    }
                }
            }
        };

        // ASR: Whisper in-process via Candle by default (multilingual, French).
        // `PARLEY_WHISPER_REPO` overrides the HF model; `stub`/`off` skips it for
        // offline dev and tests (no ~465 MB download). A load failure falls back
        // to the stub so the app still boots — the log says which happened.
        let whisper_repo = env_or("PARLEY_WHISPER_REPO", "openai/whisper-small");
        let transcriber: Arc<dyn Transcriber> = match whisper_repo.as_str() {
            "stub" | "off" => {
                tracing::warn!("transcriber: STUB (PARLEY_WHISPER_REPO={whisper_repo})");
                Arc::new(stub::StubTranscriber)
            }
            repo => match CandleWhisper::load(repo) {
                Ok(whisper) => {
                    tracing::info!(%repo, "transcriber: candle whisper (in-process)");
                    Arc::new(whisper)
                }
                Err(e) => {
                    tracing::error!(error = %e, "transcriber: STUB (whisper load failed)");
                    Arc::new(stub::StubTranscriber)
                }
            },
        };

        // TTS: only real if a voice path is given.
        let synthesizer: Arc<dyn Synthesizer> = match std::env::var("PARLEY_PIPER_VOICE") {
            Ok(path) if !path.is_empty() => {
                tracing::info!(%path, "synthesizer: piper");
                Arc::new(Piper::new(path))
            }
            _ => {
                tracing::warn!("synthesizer: STUB (set PARLEY_PIPER_VOICE to enable piper)");
                Arc::new(stub::StubSynthesizer)
            }
        };

        // Dictionary: the Wiktionnaire over its REST API by default; a stub when
        // PARLEY_DICTIONARY=stub (offline dev / tests).
        let dictionary: Arc<dyn Dictionary> = match std::env::var("PARLEY_DICTIONARY").as_deref() {
            Ok("stub") => {
                tracing::warn!("dictionary: STUB (PARLEY_DICTIONARY=stub)");
                Arc::new(stub::StubDictionary)
            }
            _ => {
                let base = WiktionaryDictionary::default_base_url();
                tracing::info!(%base, "dictionary: wiktionnaire");
                Arc::new(WiktionaryDictionary::new(base))
            }
        };

        // Memory store: MinIO/S3 if an endpoint is configured, else local files.
        // Same graceful-selection pattern as the providers above.
        let memory = select_memory_store(level);

        Self {
            transcriber,
            conversant,
            synthesizer,
            dictionary,
            memory,
            sessions: Arc::new(Mutex::new(HashMap::new())),
            language,
            level,
        }
    }

    /// The pedagogy prompt for this state's language and level, optionally steered
    /// toward an area the learner has not yet practiced.
    pub fn system_prompt(&self, steer: Option<&str>) -> String {
        pedagogy::system_prompt_steered(self.language, self.level, steer)
    }
}

/// Choose where learner memories live. MinIO/S3 when `PARLEY_S3_ENDPOINT` is set,
/// otherwise a local `PARLEY_DATA_DIR` (default `./data/memories`). Logs the choice
/// so it is obvious at startup where memories are going.
fn select_memory_store(level: Level) -> Arc<dyn MemoryStore> {
    if let Ok(endpoint) = std::env::var("PARLEY_S3_ENDPOINT") {
        if !endpoint.is_empty() {
            let cfg = S3Config {
                endpoint,
                bucket: env_or("PARLEY_S3_BUCKET", "parley-memories"),
                region: env_or("PARLEY_S3_REGION", "us-east-1"),
                access_key: env_or("AWS_ACCESS_KEY_ID", "minioadmin"),
                secret_key: env_or("AWS_SECRET_ACCESS_KEY", "minioadmin"),
            };
            tracing::info!(bucket = %cfg.bucket, "memory: minio/s3");
            return Arc::new(S3MemoryStore::new(cfg, level));
        }
    }

    let dir = env_or("PARLEY_DATA_DIR", "data/memories");
    match FsMemoryStore::new(&dir, level) {
        Ok(store) => {
            tracing::info!(%dir, "memory: local files");
            Arc::new(store)
        }
        Err(e) => {
            // Fall back to a temp dir so the app still boots; memories won't survive.
            tracing::error!(error = %e, "memory: local dir unavailable, using temp");
            let tmp = std::env::temp_dir().join("parley-memories");
            Arc::new(FsMemoryStore::new(&tmp, level).expect("temp dir writable"))
        }
    }
}

fn env_or(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_string())
}
