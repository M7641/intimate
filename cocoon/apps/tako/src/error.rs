use axum::{
    http::StatusCode,
    response::{IntoResponse, Json, Response},
};
use serde::Serialize;
use tracing::{error, warn};
use utoipa::ToSchema;

/// Internal error type — carries the full detail for server-side logging.
///
/// It is **never serialized to the client directly**. `IntoResponse` logs the
/// detail and sends back a controlled [`ErrorResponse`] instead, so internal
/// information (SQL, paths, underlying error text) cannot leak.
#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("{0}")]
    BadRequest(String),

    #[error("{0}")]
    PayloadTooLarge(String),

    #[error("{0}")]
    NotFound(String),

    /// The server is at capacity and is shedding the request (backpressure),
    /// rather than queueing it unboundedly. Maps to 503 so clients retry later.
    #[error("{0}")]
    Unavailable(String),

    #[error("{0}")]
    Internal(String),
}

impl ApiError {
    /// Build an internal error from a context message and an underlying error.
    ///
    /// The combined text is kept for logging only — it is not shown to the
    /// client — so callers are free to include SQL, paths, etc. for diagnosis.
    pub fn internal(context: &str, err: impl std::fmt::Display) -> Self {
        Self::Internal(format!("{context}: {err}"))
    }

    fn status(&self) -> StatusCode {
        match self {
            ApiError::BadRequest(_) => StatusCode::BAD_REQUEST,
            ApiError::PayloadTooLarge(_) => StatusCode::PAYLOAD_TOO_LARGE,
            ApiError::NotFound(_) => StatusCode::NOT_FOUND,
            ApiError::Unavailable(_) => StatusCode::SERVICE_UNAVAILABLE,
            ApiError::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

/// The error body sent to clients.
///
/// This is the single, controlled surface for what leaves the process. It is
/// deliberately minimal for now — a generic, harmless message for every error.
/// When we want responses to say more (and only what is safe) for a given error
/// kind, [`ErrorResponse::from_api_error`] is the one place to branch.
#[derive(Debug, Serialize, ToSchema)]
pub struct ErrorResponse {
    error: String,
}

impl ErrorResponse {
    /// Decide what the client sees for a given internal error.
    ///
    /// Today every error maps to the same harmless message. Branch on `_err`
    /// here to make the response expressive per kind, keeping internal detail out.
    fn from_api_error(_err: &ApiError) -> Self {
        Self::harmless()
    }

    /// A safe, generic message that discloses nothing about internals.
    fn harmless() -> Self {
        Self {
            error: "The request could not be processed.".to_string(),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status = self.status();

        // Single logging point: full detail stays server-side. A 5xx we own is an
        // error; a rejected request (4xx, client's fault) or a deliberate
        // load-shed (503 Unavailable, expected under load) is logged quieter.
        let owned_server_error =
            status.is_server_error() && !matches!(self, ApiError::Unavailable(_));
        if owned_server_error {
            error!(status = %status, detail = %self, "request failed");
        } else {
            warn!(status = %status, detail = %self, "request rejected");
        }

        let body = ErrorResponse::from_api_error(&self);
        (status, Json(body)).into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::to_bytes;

    async fn body_of(err: ApiError) -> (StatusCode, String) {
        let resp = err.into_response();
        let status = resp.status();
        let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        (status, String::from_utf8(bytes.to_vec()).unwrap())
    }

    const HARMLESS: &str = r#"{"error":"The request could not be processed."}"#;

    #[tokio::test]
    async fn internal_error_hides_detail_from_the_client() {
        let err = ApiError::internal(
            "COPY failed: COPY stage.secret_table FROM 's3://bucket/secret/path'",
            "connection refused",
        );
        let (status, body) = body_of(err).await;

        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(body, HARMLESS);
        // None of the internal detail may appear in the client body.
        for leak in ["COPY", "secret", "s3://", "connection refused"] {
            assert!(!body.contains(leak), "body leaked {leak:?}: {body}");
        }
    }

    #[tokio::test]
    async fn all_kinds_currently_return_the_harmless_body() {
        // Documents the v1 behaviour: every error maps to the same body. When
        // 4xx is made expressive, update this expectation in `from_api_error`.
        let cases = [
            (
                ApiError::BadRequest("schema X column Y mismatch".into()),
                StatusCode::BAD_REQUEST,
            ),
            (
                ApiError::PayloadTooLarge("file too big".into()),
                StatusCode::PAYLOAD_TOO_LARGE,
            ),
            (
                ApiError::Unavailable("at capacity".into()),
                StatusCode::SERVICE_UNAVAILABLE,
            ),
        ];
        for (err, expected_status) in cases {
            let (status, body) = body_of(err).await;
            assert_eq!(status, expected_status);
            assert_eq!(body, HARMLESS);
        }
    }
}
