# Error handling

## The principle: one error type, mapped once, leaking nothing

Handlers should return `Result<T, AppError>` where `AppError` is a single enum that
covers every failure mode. One `impl IntoResponse for AppError` decides — in one place —
which HTTP status and which JSON body each variant produces. This gives you:

- **Consistency**: every error in the service looks the same to the client.
- **Safety**: internal detail (SQL, schema names, connection strings) is logged but a
  *generic* message is returned. Database errors are the classic leak vector.
- **Ergonomics**: `?` works everywhere once you add `From` impls for upstream errors.

## The enum and its `IntoResponse`

```rust
#[derive(Debug)]
pub enum AppError {
    Validation(String),   // client sent something invalid  -> 400
    NotFound(String),     // resource doesn't exist          -> 404
    Database(DatabaseError),  // internal — never shown raw  -> 500
    Internal(String),     // catch-all bug                   -> 500
    CircuitOpen,          // dependency is down, fail fast   -> 503
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, detail) = match &self {
            AppError::Validation(msg) => {
                // Client error: log at warn, echo the message back — it's safe,
                // the client needs to know what they got wrong.
                tracing::warn!(error.kind = "validation", %msg, "Request validation failed");
                (StatusCode::BAD_REQUEST, msg.clone())
            }
            AppError::NotFound(msg) => (StatusCode::NOT_FOUND, msg.clone()),
            AppError::Database(err) => {
                // Server error: log the FULL error internally, return a generic
                // string so we never leak schema names / connection details.
                tracing::error!(error.kind = "database", error = %err, "Database error");
                (StatusCode::INTERNAL_SERVER_ERROR, "Database error".to_string())
            }
            AppError::Internal(msg) => {
                tracing::error!(error.kind = "internal", %msg, "Internal error");
                (StatusCode::INTERNAL_SERVER_ERROR, "Internal error".to_string())
            }
            AppError::CircuitOpen => (
                StatusCode::SERVICE_UNAVAILABLE,
                "Service temporarily unavailable".to_string(),
            ),
        };
        let body = serde_json::json!({ "detail": detail });
        (status, axum::Json(body)).into_response()
    }
}
```

**The asymmetry is the whole point.** Client-fault variants (validation, not-found) return
their message and log at `warn`. Server-fault variants (database, internal) return a fixed
generic string and log the real cause at `error`. A 500 body should never carry a stack
trace, a SQL fragment, or a hostname to the outside world.

## Wiring upstream errors with `From`

Add `From` impls so `?` converts automatically — handlers stay clean:

```rust
impl From<DatabaseError> for AppError {
    fn from(err: DatabaseError) -> Self {
        AppError::Database(err)
    }
}

// Now a handler reads naturally:
pub async fn handler(State(state): State<AppState>) -> Result<Json<Vec<Row>>, AppError> {
    let rows = state.blocking_query(sql).await?;  // DatabaseError -> AppError via From
    Ok(Json(rows))
}
```

## The response envelope

The house JSON shape is a flat object with a single key:

```json
{ "detail": "schema name must be alphanumeric" }
```

Pick **one** envelope and use it everywhere (`detail` here; `tako-api` uses `error`) —
clients write one error-parsing path. If you publish an OpenAPI spec, declare this shape
as an `ErrorResponse` schema so it's documented (see `testing-and-openapi.md`).

## `thiserror` vs hand-rolled

Both are in use; choose by how much the enum carries:

- **`thiserror`** when variants wrap source errors and you want `Display`/`Error` derived.
  Less boilerplate; `#[from]` generates the `From` impls.
  ```rust
  #[derive(Debug, thiserror::Error)]
  pub enum ApiError {
      #[error("{0}")] BadRequest(String),
      #[error("{0}")] Internal(String),
  }
  ```
- **Hand-rolled `Display`** (as in `service-kit`) when you want full control over the
  message and the status mapping lives entirely in `IntoResponse` anyway.

Use **`anyhow`** only at the *edges* (startup, `main`, scripts) — not in handler return
types. `anyhow::Error` doesn't implement `IntoResponse` and erases the variant you need
for status mapping. Convert it into an `AppError::Internal` at the boundary.

## A note on middleware errors

Axum handlers are infallible from tower's point of view — a `Result<T, AppError>` works
because `AppError: IntoResponse`. But a raw **tower layer** (e.g. `tower::timeout`) can
return a `BoxError` that is *not* a response. Wrap such layers with
`HandleErrorLayer` inside a `ServiceBuilder` to convert the error into a status:

```rust
ServiceBuilder::new()
    .layer(HandleErrorLayer::new(|_: BoxError| async { StatusCode::REQUEST_TIMEOUT }))
    .layer(/* the fallible tower layer */);
```

The house style sidesteps this by writing timeouts/rate-limits as `from_fn` middleware
that return a `Response` directly (see `middleware.md`) — simpler, no `HandleErrorLayer`.
