//! Entry point of the `warden` binary — the cocoon control layer.
//!
//! Initialises `tracing` (so `ouroboros`'s build/poll progress surfaces), then
//! delegates to [`warden::cli::run`]. Errors print to stderr with a non-zero exit.

mod cli;
mod ui;

use tracing_subscriber::EnvFilter;

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    std::process::exit(cli::run());
}
