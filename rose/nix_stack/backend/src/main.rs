use std::path::PathBuf;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| "nix_stack_backend=info".into()),
        )
        .init();

    let static_dir = std::env::var("STATIC_DIR").ok().map(PathBuf::from);

    let app = nix_stack_backend::app(static_dir);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000")
        .await
        .expect("failed to bind to port 3000");

    tracing::info!("listening on http://0.0.0.0:3000");

    axum::serve(listener, app).await.expect("server error");
}
