# Testing & OpenAPI

## Two levels of test, two tools

- **Fast router tests** — exercise the `Router` in-process with `tower::ServiceExt::oneshot`.
  No port, no process, no network. This is why `build_router` lives in `lib.rs` and is pure:
  tests construct an `AppState` (often with an in-memory or mocked backend), build the
  router, and fire requests at it directly.
- **Full integration tests** — spawn the real binary against a disposable, seeded database
  container and drive it over HTTP. This catches everything the unit level can't: env
  parsing, middleware order, serialization, the actual SQL dialect.

For the full testing taxonomy (unit, property, mutation, fuzz, perf gates) and how tasks
wire into moon/CI, defer to the **rust-testing-standards** skill. This file covers only the
HTTP-shaped parts specific to Axum.

## In-process router test with `oneshot`

```rust
use axum::body::Body;
use axum::http::{Request, StatusCode};
use tower::ServiceExt;   // brings `oneshot` into scope

#[tokio::test]
async fn liveness_returns_alive() {
    let state = AppState::for_test();          // cheap test state
    let app = build_router(state);

    let res = app
        .oneshot(Request::builder().uri("/health/live").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(json["status"], "alive");
}
```

`oneshot` consumes the router, runs one request through the *entire* middleware stack, and
returns the response — so this also tests that your layers don't break the route. It's the
fastest way to assert handler behaviour and status mapping.

## Full integration test against a real database

The house pattern (data_view) launches the compiled binary on a free port, pointed at a
testcontainer Postgres seeded with fixture SQL, then asserts over HTTP with `reqwest`.

```rust
#[test]
fn core_endpoints_across_wire_backends() {
    require_container_engine();

    let node = PgImage::default().start().expect("start postgres container");
    let pg_port = node.get_host_port_ipv4(5432).expect("mapped port");
    let dsn = format!("host=127.0.0.1 port={pg_port} user=postgres password=postgres dbname=postgres");
    seed(&dsn);                                  // run fixture SQL

    for warehouse in ["postgres", "amazon_redshift"] {
        let port = free_port();
        let server = start_server(port, &wire_env(warehouse, pg_port));
        assert_core_endpoints(&server.base_url, warehouse);
        // `server` drops -> child process killed; `node` drops at fn end.
    }
}
```

Two helpers carry the weight:

```rust
// Spawn the real binary, wait until it's actually serving.
pub fn start_server(port: u16, envs: &[(&str, String)]) -> Server {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_data_view"));  // path to THIS crate's binary
    cmd.arg("serve").env("DATA_VIEW_PORT", port.to_string()).env("RUST_LOG", "warn");
    for (k, v) in envs { cmd.env(k, v); }
    let child = cmd.spawn().expect("spawn server");
    let base_url = format!("http://127.0.0.1:{port}");
    wait_ready(&base_url);                        // poll /health/ready until 200
    Server { child, base_url }
}

// Poll readiness with a deadline — never sleep a fixed duration and hope.
fn wait_ready(base_url: &str) {
    let url = format!("{base_url}/health/ready");
    let client = reqwest::blocking::Client::new();
    let deadline = Instant::now() + Duration::from_secs(30);
    while Instant::now() < deadline {
        if client.get(&url).send().map(|r| r.status().is_success()).unwrap_or(false) {
            return;
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    panic!("server at {base_url} not ready within 30s");
}
```

Why this shape:
- **`env!("CARGO_BIN_EXE_<name>")`** resolves to the binary Cargo just built — no hardcoded
  path, always the current code.
- **`wait_ready` polls the readiness probe** rather than sleeping a guessed interval —
  faster on a warm machine, reliable on a cold one. This is exactly what the readiness
  endpoint is *for* in tests, mirroring what k8s does.
- **`Server` owns the child** and kills it on `Drop`, so each test cleans up even on panic.
- **Disposable containers** seeded fresh per run keep tests hermetic; see
  rust-testing-standards for the testcontainers setup and seeding helpers.

```toml
testcontainers = { version = "0.24", features = ["blocking"] }
testcontainers-modules = { version = "0.12", features = ["postgres", "blocking"] }
reqwest = { version = "0.12", features = ["blocking", "json"] }
```

## OpenAPI with utoipa

If anyone other than you consumes the API, generate an OpenAPI spec from the code so it can
never drift from reality. utoipa derives it from annotations.

Annotate each handler:

```rust
#[utoipa::path(
    get,
    path = "/api/data_view/data/{table_name}",
    params(("table_name" = String, Path, description = "Table to read")),
    responses(
        (status = 200, description = "Rows", body = Vec<Row>),
        (status = 400, description = "Invalid request", body = ErrorResponse),
    )
)]
pub async fn handler(/* ... */) -> Result<Json<Vec<Row>>, AppError> { /* ... */ }
```

Collect them into one document, including your error envelope as a schema:

```rust
#[derive(OpenApi)]
#[openapi(
    info(title = "Data View API", version = "0.1.0"),
    paths(routes::health::liveness, routes::data_view::handler /* , ... */),
    components(schemas(service_kit::error::ErrorResponse, Row /* , ... */))
)]
pub struct ApiDoc;
```

Serve it with Swagger UI, mounted on the API group:

```rust
.merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", ApiDoc::openapi()))
```

```toml
utoipa = { version = "5", features = ["axum_extras"] }
utoipa-swagger-ui = { version = "9", features = ["axum"] }
```

The payoff: the spec is generated from the same types and routes that serve traffic, so
"the docs are wrong" stops being possible — a renamed field or changed status breaks the
build, not just the documentation.
