//! Point d'entrée du binaire `ouroboros`.
//!
//! Initialise `tracing` (filtre via `RUST_LOG`) puis délègue à
//! [`ouroboros::cli::run`]. Les erreurs sont affichées sur stderr et terminent le
//! processus avec un code non nul.

use tracing_subscriber::EnvFilter;

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    if let Err(err) = ouroboros::cli::run() {
        eprintln!("Error: {err}");
        std::process::exit(1);
    }
}
