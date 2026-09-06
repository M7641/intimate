
## Part 1: Making Axum the Best It Can Be

Axum on its own is just a routing layer. To make it production-grade, you need to assemble the right ecosystem around it. Here's what a real-world Axum service looks like.

### The Foundation: Tokio Runtime

Everything in Axum sits on top of Tokio. Getting Tokio right matters.

```rust
#[tokio::main]
async fn main() {
    // For production, consider tuning the runtime
    // Default multi-threaded scheduler is usually correct
    // but you can configure worker threads explicitly:
    //
    // tokio::runtime::Builder::new_multi_thread()
    //     .worker_threads(num_cpus::get())
    //     .enable_all()
    //     .build()
}
```

The default `#[tokio::main]` is fine for most cases. The key discipline is **never blocking the async runtime** — any CPU-heavy or blocking I/O work should be offloaded to `tokio::task::spawn_blocking` or a dedicated thread pool.

### Project Structure

A well-organised Axum service separates concerns clearly:

```
service/
├── src/
│   ├── main.rs              # Entrypoint, runtime setup, server binding
│   ├── app.rs               # Router construction, middleware stacking
│   ├── config.rs            # Typed config from env/files (via `config` crate)
│   ├── errors.rs            # Unified error type implementing IntoResponse
│   ├── extractors/          # Custom Axum extractors
│   │   ├── mod.rs
│   │   ├── auth.rs          # e.g. AuthenticatedUser extractor
│   │   └── validated.rs     # Request validation wrapper
│   ├── handlers/            # Route handlers grouped by domain
│   │   ├── mod.rs
│   │   ├── health.rs
│   │   ├── users.rs
│   │   └── orders.rs
│   ├── middleware/           # Tower middleware (logging, auth, rate limiting)
│   │   ├── mod.rs
│   │   ├── request_id.rs
│   │   └── auth.rs
│   ├── models/              # Domain types, DB models
│   │   ├── mod.rs
│   │   └── user.rs
│   ├── repositories/        # Database access layer
│   │   ├── mod.rs
│   │   └── user_repo.rs
│   └── services/            # Business logic layer
│       ├── mod.rs
│       └── user_service.rs
├── migrations/              # SQLx migrations
├── proto/                   # Protobuf definitions (if this service also speaks gRPC)
├── Cargo.toml
└── .env
```

### Application State

Axum's state system is how you inject shared resources. Define a single `AppState` and share it across all routes:

```rust
use sqlx::PgPool;
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    pub redis: deadpool_redis::Pool,
    pub config: Arc<AppConfig>,
    pub http_client: reqwest::Client, // reuse a single client
}
```

Pass it to the router with `.with_state(state)`. Every handler can then extract `State<AppState>` without any global statics.

### The Middleware Stack

This is where Axum becomes genuinely powerful. Axum is built on Tower, meaning you compose middleware as layers. The order matters — outermost layers execute first on the way in, last on the way out.

```rust
use axum::{Router, middleware};
use tower::ServiceBuilder;
use tower_http::{
    trace::TraceLayer,
    cors::CorsLayer,
    compression::CompressionLayer,
    timeout::TimeoutLayer,
    request_id::{MakeRequestUuid, SetRequestIdLayer, PropagateRequestIdLayer},
};
use std::time::Duration;

pub fn build_router(state: AppState) -> Router {
    let middleware_stack = ServiceBuilder::new()
        // Outermost: assign a unique request ID to every request
        .layer(SetRequestIdLayer::x_request_id(MakeRequestUuid))
        // Propagate that ID back in the response
        .layer(PropagateRequestIdLayer::x_request_id())
        // Structured tracing for every request (method, path, status, latency)
        .layer(TraceLayer::new_for_http())
        // Kill requests that take too long
        .layer(TimeoutLayer::new(Duration::from_secs(30)))
        // Compress responses
        .layer(CompressionLayer::new())
        // CORS policy
        .layer(CorsLayer::permissive()); // tighten for production

    Router::new()
        .nest("/api/v1/users", user_routes())
        .nest("/api/v1/orders", order_routes())
        .route("/health", get(health_check))
        .layer(middleware_stack)
        .with_state(state)
}
```

For custom middleware (e.g. authentication), use `axum::middleware::from_fn_with_state`:

```rust
async fn auth_middleware(
    State(state): State<AppState>,
    mut request: Request,
    next: Next,
) -> Result<Response, AppError> {
    let token = request
        .headers()
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .ok_or(AppError::Unauthorized)?;

    let user = validate_token(&state.db, token).await?;
    request.extensions_mut().insert(user);
    Ok(next.run(request).await)
}
```

### Unified Error Handling

One of the most important patterns. Define a single error type that knows how to become an HTTP response:

```rust
use axum::response::{IntoResponse, Response};
use axum::http::StatusCode;

pub enum AppError {
    NotFound(String),
    Unauthorized,
    Validation(Vec<String>),
    Internal(anyhow::Error),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message) = match &self {
            AppError::NotFound(msg) => (StatusCode::NOT_FOUND, msg.clone()),
            AppError::Unauthorized => (StatusCode::UNAUTHORIZED, "Unauthorized".into()),
            AppError::Validation(errors) => (
                StatusCode::UNPROCESSABLE_ENTITY,
                errors.join(", "),
            ),
            AppError::Internal(err) => {
                // Log the actual error, return a generic message
                tracing::error!(?err, "Internal server error");
                (StatusCode::INTERNAL_SERVER_ERROR, "Internal error".into())
            }
        };

        let body = serde_json::json!({ "error": message });
        (status, axum::Json(body)).into_response()
    }
}

// This lets you use `?` with any anyhow-compatible error
impl<E: Into<anyhow::Error>> From<E> for AppError {
    fn from(err: E) -> Self {
        AppError::Internal(err.into())
    }
}
```

Now every handler returns `Result<impl IntoResponse, AppError>` and you get consistent error responses everywhere with zero boilerplate.

### Database: SQLx (Compile-Time Checked Queries)

SQLx is the natural pairing — async, supports Postgres natively, and can verify your SQL against the real database at compile time:

```rust
pub async fn get_user_by_id(pool: &PgPool, user_id: Uuid) -> Result<User, AppError> {
    sqlx::query_as!(
        User,
        r#"SELECT id, email, name, created_at FROM users WHERE id = $1"#,
        user_id
    )
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("User {user_id} not found")))
}
```

The `query_as!` macro checks at compile time that the SQL is valid, the columns exist, and the types map correctly. This eliminates an entire class of runtime errors.

### Observability: Tracing + OpenTelemetry

Use the `tracing` crate ecosystem throughout, wired to OpenTelemetry for export:

```rust
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};
use opentelemetry::trace::TracerProvider;
use opentelemetry_otlp::SpanExporter;

fn init_telemetry() {
    let otlp_exporter = SpanExporter::builder()
        .with_tonic()
        .build()
        .expect("Failed to create OTLP exporter");

    let provider = opentelemetry_sdk::trace::SdkTracerProvider::builder()
        .with_batch_exporter(otlp_exporter)
        .build();

    let tracer = provider.tracer("my-service");
    let otel_layer = tracing_opentelemetry::layer().with_tracer(tracer);

    tracing_subscriber::registry()
        .with(EnvFilter::from_default_env())
        .with(tracing_subscriber::fmt::layer().json())
        .with(otel_layer)
        .init();
}
```

Then in any handler or function, just use `#[tracing::instrument]` and spans propagate automatically.

### Essential Crate Ecosystem

|Purpose|Crate|Why|
|---|---|---|
|Async runtime|`tokio`|The standard, no real alternative|
|HTTP framework|`axum`|Tower-native, composable, fast|
|Middleware|`tower`, `tower-http`|Timeout, compression, CORS, tracing, rate limiting|
|Database|`sqlx`|Async, compile-time checked, Postgres-native|
|Serialisation|`serde` + `serde_json`|Universal in the Rust ecosystem|
|Validation|`validator`|Derive-based struct validation|
|Config|`config`|Layered config from env, files, defaults|
|Error handling|`anyhow` + `thiserror`|`thiserror` for library errors, `anyhow` for application|
|Tracing|`tracing` + `tracing-subscriber`|Structured, span-based observability|
|OTel export|`opentelemetry` + `tracing-opentelemetry`|Push traces/metrics to your collector|
|Redis|`deadpool-redis`|Pooled async Redis connections|
|HTTP client|`reqwest`|Async HTTP client built on hyper|
|UUID|`uuid`|v4/v7 generation, serde support|
|Time|`chrono` or `time`|Timestamp handling|

### Key Performance Practices

There are a few patterns that separate a decent Axum service from a fast one. Connection pooling is non-negotiable — let SQLx and deadpool manage your Postgres and Redis pools respectively, and create a single `reqwest::Client` at startup to reuse across all outbound HTTP calls (this reuses TCP connections and TLS sessions).

For response serialisation, `serde_json` is good but `simd-json` can give measurable speedups if you're serialising large payloads on a hot path. Similarly, if you're streaming large responses, use Axum's `Body::from_stream` rather than buffering the entire response in memory.

Be disciplined about `spawn_blocking` — if any synchronous work takes more than a few microseconds (hashing passwords with argon2, image processing, PDF generation), move it off the async runtime. A single blocking call in an async handler can stall the entire worker thread.

Finally, use `tower::load_shed` and `tower::buffer` layers in production to protect your service under load — they provide backpressure and graceful degradation rather than letting the service collapse.

---

## Part 2: gRPC Inter-Service Architecture

### Why gRPC Between Services

REST over HTTP/JSON works, but between your own services it introduces unnecessary overhead: you're serialising to text, parsing text, manually writing client SDKs, and losing type safety at the boundary. gRPC solves all of these with binary serialisation (protobuf), code-generated clients and servers, HTTP/2 multiplexing, and bidirectional streaming.

The tradeoff is that gRPC is harder to debug casually (no curl), and the tooling ecosystem is smaller. But between services you own, the benefits dominate.

### The Protobuf-First Contract

Everything starts with `.proto` files. These are the source of truth for your API — not your code, not your documentation.

```
proto/
├── user/
│   └── v1/
│       └── user_service.proto
├── order/
│   └── v1/
│       └── order_service.proto
├── common/
│   └── v1/
│       └── types.proto        # Shared types (pagination, timestamps, etc.)
└── buf.yaml                   # Buf configuration for linting/breaking change detection
```

A typical service definition:

```protobuf
syntax = "proto3";
package user.v1;

import "common/v1/types.proto";

service UserService {
  // Unary: simple request/response
  rpc GetUser(GetUserRequest) returns (GetUserResponse);
  rpc CreateUser(CreateUserRequest) returns (CreateUserResponse);
  rpc ListUsers(ListUsersRequest) returns (ListUsersResponse);

  // Server streaming: service pushes multiple responses
  rpc WatchUserEvents(WatchUserEventsRequest) returns (stream UserEvent);
}

message GetUserRequest {
  string user_id = 1;
}

message GetUserResponse {
  User user = 1;
}

message User {
  string id = 1;
  string email = 2;
  string name = 3;
  google.protobuf.Timestamp created_at = 4;
}

message ListUsersRequest {
  int32 page_size = 1;
  string page_token = 2;
}

message ListUsersResponse {
  repeated User users = 1;
  string next_page_token = 2;
}
```

### The Architecture: What It Actually Looks Like

Here's a concrete multi-service system and how gRPC connects it all:

```
                    ┌──────────────────────┐
                    │     API Gateway       │
                    │   (Axum + REST/JSON)  │
                    │   Public-facing HTTP  │
                    └──────┬───────────────┘
                           │
              ┌────────────┼────────────────┐
              │ gRPC       │ gRPC           │ gRPC
              ▼            ▼                ▼
     ┌────────────┐ ┌────────────┐  ┌────────────────┐
     │   User     │ │   Order    │  │  Notification  │
     │  Service   │ │  Service   │  │    Service     │
     │            │ │            │  │                │
     │  Postgres  │ │  Postgres  │  │     Redis      │
     └────────────┘ └─────┬──────┘  └────────────────┘
                          │                  ▲
                          │ gRPC             │ NATS
                          ▼                  │
                   ┌────────────┐            │
                   │  Payment   │────────────┘
                   │  Service   │  (publishes payment events)
                   │            │
                   │  Postgres  │
                   └────────────┘
```

The key patterns here:

**API Gateway → Services (gRPC):** The gateway is the only thing that speaks HTTP/JSON to the outside world. Internally, it translates to gRPC calls. This means your internal services never deal with REST semantics, header parsing, or JSON.

**Service → Service (gRPC):** When the Order Service needs to validate a user exists, it calls the User Service directly over gRPC. This is a synchronous dependency — the order creation blocks until the user is confirmed.

**Events via NATS (async):** When the Payment Service processes a payment, it doesn't call the Notification Service directly. Instead it publishes a `payment.completed` event to NATS. The Notification Service subscribes and reacts. This keeps services decoupled for operations that don't need an immediate response.

The rule of thumb: **gRPC for queries and commands that need a response. NATS for events and fire-and-forget.**

### Implementation in Rust: Tonic

Tonic is the gRPC library for Rust, built on the same Tokio + Hyper stack as Axum. They share a runtime, which means you can even serve both HTTP/JSON and gRPC from the same process if needed.

**Server side:**

```rust
use tonic::{Request, Response, Status};
use user_proto::user_service_server::{UserService, UserServiceServer};
use user_proto::{GetUserRequest, GetUserResponse, User};

pub struct UserServiceImpl {
    db: PgPool,
}

#[tonic::async_trait]
impl UserService for UserServiceImpl {
    async fn get_user(
        &self,
        request: Request<GetUserRequest>,
    ) -> Result<Response<GetUserResponse>, Status> {
        let user_id = request.into_inner().user_id;

        let user = sqlx::query_as!(...)
            .fetch_optional(&self.db)
            .await
            .map_err(|e| Status::internal(e.to_string()))?
            .ok_or_else(|| Status::not_found("User not found"))?;

        Ok(Response::new(GetUserResponse {
            user: Some(user.into()),
        }))
    }
}
```

**Client side (from another service):**

```rust
use user_proto::user_service_client::UserServiceClient;
use user_proto::GetUserRequest;

pub struct UserClient {
    client: UserServiceClient<tonic::transport::Channel>,
}

impl UserClient {
    pub async fn connect(addr: &str) -> Result<Self, Box<dyn std::error::Error>> {
        // Channel handles connection pooling and reconnection
        let client = UserServiceClient::connect(addr.to_string()).await?;
        Ok(Self { client })
    }

    pub async fn get_user(&self, user_id: &str) -> Result<User, Status> {
        let mut client = self.client.clone(); // cheap clone, shares the channel
        let response = client
            .get_user(GetUserRequest {
                user_id: user_id.to_string(),
            })
            .await?;

        response
            .into_inner()
            .user
            .ok_or_else(|| Status::internal("Empty user in response"))
    }
}
```

### Proto Management with Buf

Raw `protoc` is painful. Buf replaces it with a modern toolchain:

```yaml
# buf.yaml - at the root of your proto/ directory
version: v2
modules:
  - path: proto
lint:
  use:
    - STANDARD       # Enforces naming conventions, field numbering, etc.
breaking:
  use:
    - FILE           # Catches breaking changes between commits
```

Run `buf lint` in CI to catch bad proto design. Run `buf breaking --against .git#branch=main` to prevent accidental breaking changes. This is critical — a breaking proto change silently deployed will take down calling services.

### Code Generation Pipeline

Each service generates its own client/server code from the shared protos. In Rust with Tonic, this happens at build time:

```rust
// build.rs
fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_build::configure()
        .build_server(true)
        .build_client(true)
        .compile_protos(
            &["proto/user/v1/user_service.proto"],
            &["proto/"],
        )?;
    Ok(())
}
```

For polyglot environments (e.g. a TypeScript frontend team that needs types), Buf can generate code for multiple languages from the same protos using `buf generate` with plugins for TypeScript, Go, Python, etc.

### Critical Patterns for Production

**Interceptors (middleware for gRPC):** Tonic supports Tower layers, so you get the same middleware composition as Axum. Use this for authentication, logging, and trace propagation:

```rust
let layer = tower::ServiceBuilder::new()
    .layer(tonic::service::interceptor(auth_interceptor))
    .layer(TraceLayer::new_for_grpc())
    .into_inner();

Server::builder()
    .layer(layer)
    .add_service(UserServiceServer::new(user_service))
    .serve(addr)
    .await?;
```

**Metadata propagation:** gRPC metadata is the equivalent of HTTP headers. Always propagate trace IDs and authentication tokens through metadata so distributed tracing works end-to-end. OpenTelemetry's Tonic integration handles trace context propagation automatically.

**Deadlines, not timeouts:** gRPC has a concept of deadlines that propagate across service boundaries. If the gateway sets a 5-second deadline, and the Order Service spends 3 seconds before calling the User Service, the User Service automatically inherits a 2-second deadline. This prevents cascading slow requests.

**Retries and hedging:** Configure retry policies at the client level for idempotent calls. Tonic doesn't have built-in retry policies yet, but you can use Tower's `retry` layer or implement it with a simple middleware. Only retry on `UNAVAILABLE` and `DEADLINE_EXCEEDED` — never on `INVALID_ARGUMENT` or `NOT_FOUND`.

**Health checking:** gRPC has a standard health checking protocol (`grpc.health.v1.Health`). Implement it on every service so Kubernetes liveness/readiness probes can use `grpc_health_probe` rather than falling back to HTTP sidecar hacks.

**Load balancing:** gRPC uses HTTP/2, which multiplexes all requests over a single TCP connection. This means L4 load balancers (like a basic Kubernetes Service) won't distribute requests evenly — all requests hit the same pod. You need either client-side load balancing (Tonic supports this) or an L7-aware proxy like Envoy or Linkerd.

### Versioning Strategy

Version your proto packages (`user.v1`, `user.v2`) and maintain backward compatibility within a version. The rules are simple: never remove or renumber existing fields, only add new ones. When you need a breaking change, create a `v2` package and run both versions simultaneously until all callers have migrated.

Buf's breaking change detection in CI makes this enforceable rather than aspirational.

---

## Summary

Axum becomes production-grade when you surround it with the right ecosystem: Tower middleware for cross-cutting concerns, SQLx for type-safe database access, a unified error type, and tracing wired to OpenTelemetry. The framework itself is deliberately minimal — the power comes from how composable everything is.

gRPC becomes the backbone of your service mesh when you treat protos as the source of truth, enforce them with Buf, use Tonic's Tower integration for middleware, and respect HTTP/2's load balancing requirements. The combination of typed contracts, binary serialisation, deadline propagation, and streaming gives you a communication layer that's both faster and safer than REST between services.

Together, an Axum gateway translating REST to gRPC, with Tonic-powered services behind it, gives you a system that's fast, type-safe from edge to database, and debuggable through distributed tracing.