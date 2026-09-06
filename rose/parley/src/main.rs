mod cli;

use clap::Parser;

use parley::app;
use parley::state::AppState;

/// Boot the API server: tracing, provider selection, router, then serve.
///
/// Invoked by the `serve` command (the default). The frontend orchestration
/// lives in the other CLI commands (`src/cli.rs`).
async fn start_server() -> anyhow::Result<()> {
    // RUST_LOG controls verbosity; default to info for our crate.
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "parley=info,tower_http=info".into()),
        )
        .init();

    let state = AppState::from_env();
    let router = app(state);

    // Port is fixed in normal operation; `PARLEY_PORT` overrides it so an
    // integration test (or a second instance) can bind a free port.
    let port = std::env::var("PARLEY_PORT").unwrap_or_else(|_| "3000".to_string());
    let addr = format!("0.0.0.0:{port}");
    let listener = tokio::net::TcpListener::bind(&addr).await?;

    // Point the operator at whichever URL is actually live: the SPA if a bundle
    // was built (`parley build`), otherwise API-only and the Vite dev server.
    match parley::spa_bundle_dir() {
        Some(dir) => {
            tracing::info!("serving frontend from {}", dir.display());
            tracing::info!("open the app at http://localhost:{port}");
        }
        None => tracing::info!(
            "no frontend bundle — serving API only at http://localhost:{port} \
             (run `parley dev` or `parley start` for the Vite UI on :5173)"
        ),
    }

    axum::serve(listener, router).await?;
    Ok(())
}

/// Entry point: parse the CLI, then dispatch. `serve` spins up the tokio
/// runtime; the other commands run synchronously (they only spawn processes).
fn main() {
    let cli = cli::Cli::parse();
    // No subcommand defaults to `serve` — the production entrypoint.
    match cli.command.unwrap_or(cli::CliCommand::Serve) {
        cli::CliCommand::Serve => {
            let runtime = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .expect("failed to build tokio runtime");
            if let Err(err) = runtime.block_on(start_server()) {
                tracing::error!("parley failed to start: {err}");
                std::process::exit(1);
            }
        }
        command => std::process::exit(cli::dispatch(command)),
    }
}
