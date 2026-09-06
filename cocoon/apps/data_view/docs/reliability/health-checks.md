# Health Checks

> Status: **Implemented** — `src/routes/health.rs`

## What

Two HTTP endpoints that report whether the service is alive and whether it can serve traffic:

| Endpoint            | Name      | Checks                                        |
| ------------------- | --------- | --------------------------------------------- |
| `GET /health/live`  | Liveness  | Process is running (always 200)               |
| `GET /health/ready` | Readiness | Database reachable, pool healthy (200 or 503) |

## Why

A single `/health` endpoint that always returns 200 is the most common anti-pattern in production services. It gives false confidence:

| Failure mode              | Single `/health` that returns 200                           | Liveness + Readiness                                                                                            |
| ------------------------- | ----------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------- |
| DB is down                | Health says "ok", users get 500s                            | Readiness returns 503 → load balancer stops routing → users see "service unavailable" instead of cryptic errors |
| Process deadlocked        | Health handler never responds → timeout after 30s           | Liveness fails → orchestrator kills and restarts the pod                                                        |
| Pool exhausted            | Health says "ok" (it doesn't check the pool)                | Readiness reports `active: 10, idle: 0` → operator sees the problem in the response body                        |
| Cold start (pool warming) | Health says "ok" before the first connection is established | Readiness returns 503 until the DB ping succeeds → no premature traffic                                         |

Kubernetes requires these two probes to operate correctly. Without a readiness probe, a pod receives traffic the instant it starts — before DB connections are established. Without a liveness probe, a deadlocked pod stays in rotation indefinitely.

## How — This Repo

### Liveness (`GET /health/live`)

```rust
pub async fn liveness() -> Json<serde_json::Value> {
    Json(serde_json::json!({ "status": "alive" }))
}
```

Intentionally does NOT check any dependencies. If the process can execute this function, it is alive. A transient DB outage should not trigger a restart — that would cause a restart storm that hammers the recovering database.

### Readiness (`GET /health/ready`)

```rust
pub async fn readiness(State(state): State<AppState>) -> Result<Json<...>, StatusCode> {
    let ping_result = tokio::task::spawn_blocking(move || s.ping()).await;

    match ping_result {
        Ok(()) => Ok(Json(json!({
            "status": "ready",
            "checks": {
                "database": { "status": "up", "latency_ms": latency_ms },
                "pool": {
                    "active": pool.connections - pool.idle_connections,
                    "idle": pool.idle_connections,
                    "total": pool.connections,
                }
            }
        }))),
        Err(e) => Err(StatusCode::SERVICE_UNAVAILABLE),
    }
}
```

Returns 503 when the database is unreachable. Includes latency and pool stats so operators can see health details without separate tooling.

### Kubernetes configuration

```yaml
livenessProbe:
  httpGet:
    path: /health/live
    port: 8050
  initialDelaySeconds: 5
  periodSeconds: 10
  failureThreshold: 3 # restart after 30s of no response

readinessProbe:
  httpGet:
    path: /health/ready
    port: 8050
  initialDelaySeconds: 10
  periodSeconds: 5
  failureThreshold: 2 # remove from LB after 10s of failures
```
