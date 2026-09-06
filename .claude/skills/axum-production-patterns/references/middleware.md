# The middleware stack

Middleware is where cross-cutting production concerns live. Axum gives you two ways to add
it; the house style uses both deliberately.

## Two mechanisms

- **`tower-http` layers** — battle-tested implementations of standard concerns. Prefer
  these over writing your own: `TraceLayer`, `CorsLayer`, `CompressionLayer`,
  `CatchPanicLayer`, `DefaultBodyLimit`, `TimeoutLayer`.
- **`axum::middleware::from_fn`** (and `from_fn_with_state`) — write an `async fn` that
  takes the request, calls `next.run(req)`, and returns a `Response`. Use this for
  app-specific logic: request IDs, RED metrics, tiered rate limiting, security headers.
  `from_fn` middleware return a `Response` directly, so they never need `HandleErrorLayer`.

## Order (recap)

`.layer()` wraps inside-out: the **last** `.layer()` is the **outermost**. House order,
outer→inner:

```
CatchPanic → SecurityHeaders → RequestId → Metrics → Trace → CORS → Compression → [Timeout, RateLimit] → Handler
```

Cross-cutting safety on the outside (catch panics, stamp an ID, record metrics for *every*
request including failures); per-route concerns (timeout, rate limit) closest to the
handler so infra routes can opt out by being merged separately.

## tower-http layers — configuration that matters

```rust
// CORS: explicit origins from config. NEVER Any + credentials together.
fn build_cors_layer() -> CorsLayer {
    let origins = std::env::var("CORS_ALLOWED_ORIGINS")
        .unwrap_or_else(|_| "http://localhost:5173".to_string());
    let origins: Vec<HeaderValue> = origins
        .split(',')
        .filter_map(|o| o.trim().parse().ok())
        .collect();
    CorsLayer::new()
        .allow_origin(origins)
        .allow_methods([Method::GET, Method::POST])
        .allow_headers([header::CONTENT_TYPE, header::AUTHORIZATION])
}

// Body limit: reject oversized requests before reading them into memory.
.layer(DefaultBodyLimit::max(10 * 1024 * 1024))   // 10 MB

// Panics -> 500 instead of a dropped connection (which clients see as a hang).
.layer(CatchPanicLayer::new())

// Compression: gzip responses.
.layer(CompressionLayer::new())

// Tracing: structured span per request, status + latency on completion.
.layer(TraceLayer::new_for_http())
```

`Cargo.toml` features needed:
```toml
tower-http = { version = "0.6", features = ["trace", "cors", "compression-gzip", "catch-panic", "timeout", "fs"] }
```

## Custom middleware: request ID

Stamp every request with an ID (reuse an incoming `x-request-id` or generate one), put it
in a tracing span so *all* downstream logs carry it, and echo it back on the response so a
client can quote it in a bug report.

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
    .instrument(span)   // every log inside the handler inherits request_id
    .await
}
```

## Custom middleware: RED metrics

Record Rate, Errors, Duration for every request. **Use the matched route pattern, not the
raw path**, as the metric label — otherwise `/users/1`, `/users/2`, … explode metric
cardinality and melt Prometheus.

```rust
pub async fn metrics_layer(matched: Option<MatchedPath>, req: Request, next: Next) -> Response {
    let method = req.method().to_string();
    let path = matched
        .map(|p| p.as_str().to_string())          // "/users/{id}", not "/users/1"
        .unwrap_or_else(|| "unmatched".to_string());

    let start = Instant::now();
    let response = next.run(req).await;
    let status = response.status().as_u16().to_string();

    metrics::counter!("http_requests_total",
        "method" => method.clone(), "path" => path.clone(), "status" => status).increment(1);
    metrics::histogram!("http_request_duration_seconds",
        "method" => method, "path" => path).record(start.elapsed().as_secs_f64());

    response
}
```

## Custom middleware: timeout

Bound every request. A handler that hangs forever holds a connection and a pool slot; under
load that cascades into exhaustion. Return 504 when the budget is blown.

```rust
pub async fn global_timeout(req: Request, next: Next) -> Response {
    let secs = env_parse("GLOBAL_TIMEOUT_SECS", 30);
    match tokio::time::timeout(Duration::from_secs(secs), next.run(req)).await {
        Ok(response) => response,
        Err(_) => (StatusCode::GATEWAY_TIMEOUT, Json(json!({ "detail": "Request timed out" }))).into_response(),
    }
}
```

Two tiers in practice: a default (30s) for normal routes and a `heavy_route_timeout`
(120s) applied only to known-slow endpoints. Apply them at the route-group level so infra
probes never time out.

## Custom middleware: tiered rate limiting

Needs shared state (the limiter), so use `from_fn_with_state`. Return 429 with a
`retry-after` header so well-behaved clients back off.

```rust
pub async fn rate_limit(State(state): State<AppState>, req: Request, next: Next) -> Response {
    if state.rate_limiter().check() {
        next.run(req).await
    } else {
        (
            StatusCode::TOO_MANY_REQUESTS,
            [(header::RETRY_AFTER, "1")],
            Json(json!({ "detail": "Rate limit exceeded" })),
        ).into_response()
    }
}
```

House limiter is a custom fixed-window counter (lock-free atomics, background reset task),
not a crate — see `service-kit/src/rate_limiter.rs`. Two buckets: global
(`RATE_LIMIT_RPS`, default 100) and heavy (`RATE_LIMIT_HEAVY_RPS`, default 10). **Infra
routes are exempt** by virtue of being merged into the router separately, without this layer.

## Custom middleware: security headers

Cheap defense-in-depth. Add on every response:

```rust
pub async fn security_headers(req: Request, next: Next) -> Response {
    let mut res = next.run(req).await;
    let h = res.headers_mut();
    h.insert("x-content-type-options", HeaderValue::from_static("nosniff"));
    h.insert("x-frame-options", HeaderValue::from_static("deny"));
    h.insert("referrer-policy", HeaderValue::from_static("no-referrer"));
    res
}
```

## Authentication (when you need it)

The core house services have no auth (internal, network-isolated). When you do add it,
the idiomatic place is a **custom extractor** that validates a JWT / API key and yields the
authenticated principal — failing extraction returns 401 via `IntoResponse`:

```rust
pub struct AuthUser { pub id: String }

impl<S: Send + Sync> FromRequestParts<S> for AuthUser {
    type Rejection = AppError;
    async fn from_request_parts(parts: &mut Parts, _: &S) -> Result<Self, Self::Rejection> {
        let token = bearer_token(parts).ok_or(AppError::Validation("missing token".into()))?;
        let claims = verify_jwt(&token).map_err(|_| AppError::Validation("invalid token".into()))?;
        Ok(AuthUser { id: claims.sub })
    }
}
```

A handler then just takes `auth: AuthUser` and is guaranteed authenticated. This composes
better than a blanket middleware because protected handlers declare the requirement in
their signature, and `axum-extra`'s `TypedHeader<Authorization<Bearer>>` can extract the
token for you.
