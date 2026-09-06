//! augur — a local, low-latency completion server for Zed's edit prediction.
//!
//! Zed's edit prediction can point at any server speaking the OpenAI
//! `/v1/completions` shape. `augur` is that server: it loads a quantized Qwen3
//! GGUF once into memory (on Metal, on Apple Silicon) and serves raw text
//! completions over a single shared model instance.
//!
//! Run it with Metal — CPU inference of a 4B model is unusably slow:
//!     cargo run -p augur --features metal --release -- serve

mod completions;
mod gguf_tokenizer;
mod model;

use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};

use crate::model::{CandleModel, ModelConfig};

/// Shared across requests. The `Mutex` serializes inference: with one model
/// instance and a single user, requests run one at a time — which also avoids
/// fighting over the laptop's memory bandwidth. Inference runs inside
/// `spawn_blocking`, so holding this lock never blocks the async runtime.
#[derive(Clone)]
pub struct AppState {
    model: Arc<Mutex<CandleModel>>,
    model_name: String,
    /// When set, each request's raw prompt and completion are appended here as
    /// JSONL — the way to see exactly what Zed sends and align the format.
    prompt_log: Option<std::path::PathBuf>,
}

#[derive(Parser)]
#[command(
    name = "augur",
    about = "Local edit-prediction completion server for Zed"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Load the model and serve `/v1/completions`.
    Serve(ServeArgs),
}

#[derive(Parser)]
struct ServeArgs {
    #[arg(long, default_value = "127.0.0.1")]
    host: String,

    #[arg(long, default_value_t = 8065)]
    port: u16,

    /// GGUF weights URL. Defaults to Sweep Next-Edit 1.5B (Qwen2.5-Coder based,
    /// purpose-built for next-edit prediction). Any Qwen2/Qwen3 GGUF works.
    #[arg(
        long,
        env = "AUGUR_MODEL_URL",
        default_value = "https://huggingface.co/sweepai/sweep-next-edit-1.5B/resolve/main/sweep-next-edit-1.5b.q8_0.v2.gguf"
    )]
    model_url: String,

    /// Cache filename for the downloaded GGUF.
    #[arg(
        long,
        env = "AUGUR_MODEL_FILE",
        default_value = "sweep-next-edit-1.5b.q8_0.v2.gguf"
    )]
    model_file: String,

    /// Optional external tokenizer JSON URL. Omit it (the default) to rebuild
    /// the tokenizer from the GGUF's own vocabulary, which always matches the
    /// weights — needed for models like Sweep that ship no `tokenizer.json`.
    #[arg(long, env = "AUGUR_TOKENIZER_URL")]
    tokenizer_url: Option<String>,

    /// Reported back in completion responses as the `model` field.
    #[arg(long, env = "AUGUR_MODEL_NAME", default_value = "sweep-next-edit-1.5b")]
    model_name: String,

    /// Append each request's raw prompt + completion to
    /// `~/.cache/augur/prompts.jsonl`. Use it to capture exactly what Zed sends
    /// and align the prompt format to the model.
    #[arg(long)]
    log_prompts: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt().with_target(false).init();

    match Cli::parse().command {
        Command::Serve(args) => serve(args).await,
    }
}

async fn serve(args: ServeArgs) -> Result<()> {
    let cfg = ModelConfig {
        model_url: args.model_url,
        model_file: args.model_file,
        tokenizer_url: args.tokenizer_url,
    };

    // Load on a blocking thread: download + GGUF parse + dequantize are heavy
    // and must not stall the async runtime as it starts up.
    let model = tokio::task::spawn_blocking(move || CandleModel::load(&cfg))
        .await
        .context("model load task panicked")??;

    let prompt_log = args.log_prompts.then(|| {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        let path = std::path::PathBuf::from(home).join(".cache/augur/prompts.jsonl");
        tracing::info!("logging prompts to {}", path.display());
        path
    });

    let state = AppState {
        model: Arc::new(Mutex::new(model)),
        model_name: args.model_name,
        prompt_log,
    };

    let app = completions::router(state);

    let addr: SocketAddr = format!("{}:{}", args.host, args.port)
        .parse()
        .context("invalid host/port")?;
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .with_context(|| format!("cannot bind {addr}"))?;

    tracing::info!("augur listening on http://{addr}  (POST /v1/completions)");
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .context("server error")?;
    Ok(())
}

async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
    tracing::info!("shutting down");
}
