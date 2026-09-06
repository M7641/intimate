# Request Correlation

> Status: **Implemented** — `src/middleware.rs`

## What

Every HTTP request is assigned a unique identifier (UUID v4). This ID is:

1. Injected into a `tracing` span so every log line carries it
2. Stored in request extensions so handlers can access it
3. Returned in the `x-request-id` response header so the client can reference it

If the caller already provides an `x-request-id` header (e.g. from an API gateway or frontend), it is reused instead of generating a new one.

## Why

Without request IDs, debugging a production issue looks like this:

```
ERROR database error: connection refused
ERROR database error: connection refused
ERROR database error: connection refused
```

Three errors. Are they three users? One user retrying? Which endpoint? You don't know.

With request IDs:

```json
{"level":"ERROR","request_id":"a1b2c3","path":"/api/data_view/columns/users","error.kind":"database"}
{"level":"ERROR","request_id":"d4e5f6","path":"/api/data_view/schemas","error.kind":"database"}
{"level":"ERROR","request_id":"a1b2c3","path":"/api/data_view/columns/users","error.kind":"database"}
```

Now you see: `a1b2c3` hit `/columns/users` twice (a retry), and `d4e5f6` is a different user on `/schemas`. Both hit a DB error — the database is likely down. This took 5 seconds to diagnose instead of 5 minutes.

When a user reports an error, you ask for the `x-request-id` from the response header and search:

```bash
cat logs.json | jq 'select(.spans[].request_id == "a1b2c3")'
```

## How — This Repo

### Middleware (`src/middleware.rs`)

```rust
pub async fn request_id(mut req: Request, next: Next) -> Response {
    let id = req
        .headers()
        .get("x-request-id")
        .and_then(|v| v.to_str().ok())
        .map(String::from)
        .unwrap_or_else(|| Uuid::new_v4().to_string());

    req.extensions_mut().insert(RequestId(id.clone()));

    let span = tracing::info_span!("request", request_id = %id);

    async move {
        let mut res = next.run(req).await;
        if let Ok(val) = id.parse() {
            res.headers_mut().insert("x-request-id", val);
        }
        res
    }
    .instrument(span)
    .await
}
```

### Key design decisions

- **Reuse incoming IDs**: enables distributed tracing across services. A frontend or API gateway initiates the ID and all downstream services propagate it.
- **`tracing::Instrument`** instead of `span.enter()`: in async Rust, `span.enter()` is unsafe across `.await` points because the guard may be active when the task yields. `.instrument(span)` correctly associates the span with the Future.
- **Extensions**: the `RequestId` newtype is stored in request extensions so any handler can extract it without parsing headers.
