use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use database::DatabaseError;
use utoipa::ToSchema;

/// Error response body returned on failure.
#[derive(serde::Serialize, ToSchema)]
pub struct ErrorResponse {
    pub detail: String,
}

#[derive(Debug)]
#[allow(dead_code)]
pub enum AppError {
    Validation(String),
    NotFound(String),
    Database(DatabaseError),
    Internal(String),
    CircuitOpen,
}

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AppError::Validation(msg) => write!(f, "{msg}"),
            AppError::NotFound(msg) => write!(f, "{msg}"),
            AppError::Database(err) => write!(f, "{err}"),
            AppError::Internal(msg) => write!(f, "{msg}"),
            AppError::CircuitOpen => write!(f, "Circuit breaker is open"),
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, detail) = match &self {
            AppError::Validation(msg) => {
                tracing::warn!(error.kind = "validation", %msg, "Request validation failed");
                (StatusCode::BAD_REQUEST, msg.clone())
            }
            AppError::NotFound(msg) => {
                tracing::warn!(error.kind = "not_found", %msg, "Resource not found");
                (StatusCode::NOT_FOUND, msg.clone())
            }
            AppError::Database(err) => {
                // Log the full error internally but return a generic message to
                // the client to avoid leaking schema names, connection details, etc.
                tracing::error!(error.kind = "database", error = %err, "Database error");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Database error".to_string(),
                )
            }
            AppError::Internal(msg) => {
                tracing::error!(error.kind = "internal", %msg, "Internal server error");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Internal server error".to_string(),
                )
            }
            AppError::CircuitOpen => {
                tracing::error!(
                    error.kind = "circuit_open",
                    "Circuit breaker open — rejecting request"
                );
                (
                    StatusCode::SERVICE_UNAVAILABLE,
                    "Service temporarily unavailable — database circuit breaker is open"
                        .to_string(),
                )
            }
        };

        let body = serde_json::json!({ "detail": detail });
        (status, axum::Json(body)).into_response()
    }
}

impl From<DatabaseError> for AppError {
    fn from(err: DatabaseError) -> Self {
        AppError::Database(err)
    }
}
