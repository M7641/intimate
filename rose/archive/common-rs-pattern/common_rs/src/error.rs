//! Shared HTTP error type. Maps domain errors (sqlx, validation) onto
//! the four status codes the JSON API exposes: 400, 404, 409, 500.
//!
//! `Internal` always renders the user-facing message as a generic
//! "Internal server error" so internal details don't leak through the
//! response body — the original message goes to `tracing::error!` for
//! operators only.

use axum::{Json, http::StatusCode, response::IntoResponse};
use serde_json::json;
use thiserror::Error;

use crate::db::DbError;
use crate::validation::ValidationError;

#[derive(Debug, Error)]
pub enum ApiError {
    #[error("{0}")]
    BadRequest(String),

    #[error("{0}")]
    NotFound(String),

    #[error("{0}")]
    Conflict(String),

    #[error("{0}")]
    Internal(String),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        let (status, msg) = match &self {
            Self::BadRequest(m) => (StatusCode::BAD_REQUEST, m.clone()),
            Self::NotFound(m) => (StatusCode::NOT_FOUND, m.clone()),
            Self::Conflict(m) => (StatusCode::CONFLICT, m.clone()),
            Self::Internal(m) => {
                tracing::error!(error = %m, "internal error");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Internal server error".to_string(),
                )
            }
        };
        (status, Json(json!({ "detail": msg }))).into_response()
    }
}

impl From<DbError> for ApiError {
    fn from(e: DbError) -> Self {
        Self::Internal(format!("db error: {e}"))
    }
}

impl From<ValidationError> for ApiError {
    fn from(e: ValidationError) -> Self {
        Self::BadRequest(e.to_string())
    }
}

impl From<sqlx::Error> for ApiError {
    fn from(e: sqlx::Error) -> Self {
        Self::Internal(format!("sqlx: {e}"))
    }
}

pub type ApiResult<T> = Result<T, ApiError>;
