//! `service-kit` — scaffolding HTTP partagé par les services axum de cocoon.
//!
//! Regroupe l'état applicatif, la gestion d'erreurs, les middlewares, le pool de
//! connexions (au-dessus de [`database`]), le rate limiting et le circuit
//! breaker, et le signal d'arrêt gracieux. Extrait depuis le code dupliqué de
//! `data_view` et `warehouse`.
//!
//! Les features backend (`postgres`, `duckdb`, …) sont choisies par l'application
//! finale via sa propre dépendance à `database` : ce crate reste agnostique.
pub mod circuit_breaker;
pub mod cli_ui;
pub mod error;
pub mod layers;
pub mod metrics;
pub mod middleware;
pub mod openapi;
pub mod pool;
pub mod rate_limiter;
pub mod shutdown;
pub mod state;

// Ré-exports de confort : `service_kit::AppState` plutôt que `service_kit::state::AppState`.
pub use circuit_breaker::CircuitBreaker;
pub use error::{AppError, ErrorResponse};
pub use middleware::RequestId;
pub use rate_limiter::RateLimiter;
pub use shutdown::shutdown_signal;
pub use state::AppState;
