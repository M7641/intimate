# API Documentation

## What

An OpenAPI 3.0 specification generated from the code at compile time, served as an interactive Swagger UI and a machine-readable JSON schema.

## Why

API documentation is not optional for a production service. It is the contract between the service and its consumers.

## How — This Repo

### Schema generation (`src/openapi.rs`)

```rust
#[derive(OpenApi)]
#[openapi(
    info(title = "Data View API", version = "0.1.0"),
    paths(
        routes::health::liveness,
        routes::health::readiness,
        routes::health::metrics_handler,
        routes::data_view::browse::schemas_handler,
        // ... all endpoints listed
    ),
    components(schemas(
        crate::error::ErrorResponse,
        routes::data_view::inspect::ColumnInfoResp,
        // ... all response types listed
    ))
)]
pub struct ApiDoc;
```

The `utoipa` crate reads `#[utoipa::path]` annotations on handlers and `#[derive(ToSchema)]` on types to generate the full OpenAPI spec at compile time. No runtime overhead.

### Handler annotations

```rust
#[utoipa::path(
    get,
    path = "/health/ready",
    tag = "Health",
    responses(
        (status = 200, description = "Service is ready", body = serde_json::Value),
        (status = 503, description = "Service is not ready")
    )
)]
pub async fn readiness(...) { ... }
```

### Access points

| URL                      | Content                                               |
| ------------------------ | ----------------------------------------------------- |
| `/swagger-ui`            | Interactive Swagger UI — try endpoints in the browser |
| `/api-docs/openapi.json` | Machine-readable OpenAPI 3.0 JSON spec                |

### Key principles

1. **Documentation lives next to the code**: handler annotations are on the same function they document. Moving or renaming the function updates the docs automatically.
2. **Response types are schema-derived**: `#[derive(Serialize, ToSchema)]` on structs means the response schema always matches the actual serialisation.
3. **Error responses are documented**: consumers know exactly what error shape to expect (`{ "detail": "..." }`).
