# Request Body Size Limits

> Status: **Implemented**

## What

A maximum size enforced on incoming request bodies to prevent memory exhaustion from oversized payloads.

## Why

| Without body limit                                               | With body limit                                      |
| ---------------------------------------------------------------- | ---------------------------------------------------- |
| Client sends 10 GB POST → server allocates 10 GB → OOM kill      | Request rejected at 10 MB with 413 Payload Too Large |
| Slow loris variant: trickle a massive body → tie up a connection | Connection freed after limit exceeded                |

## How

Axum's `DefaultBodyLimit` layer is applied globally in `src/app.rs`:

```rust
const MAX_BODY_SIZE: usize = 10 * 1024 * 1024; // 10 MB

.layer(axum::extract::DefaultBodyLimit::max(MAX_BODY_SIZE))
```

This applies to all routes. Since this service is primarily read-only (GET endpoints), 10 MB is generous — most requests have no body at all. The limit protects against accidental or malicious large payloads on any endpoint that accepts a body.

## Configuration

Currently hardcoded at 10 MB. If per-route limits are needed in the future, use `.route_layer(DefaultBodyLimit::max(...))` on specific route groups.
