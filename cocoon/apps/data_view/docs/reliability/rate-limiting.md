# Rate Limiting

> Status: **Implemented**

## What

Limiting the number of requests a client can make in a given time window. Protects the service from abuse, misbehaving scripts, and accidental DDoS from retry loops.

## Why

| Threat                                                 | Without rate limiting                                                  | With rate limiting                                                      |
| ------------------------------------------------------ | ---------------------------------------------------------------------- | ----------------------------------------------------------------------- |
| Runaway script hitting `/api/data_view/data` in a loop | Each request triggers a DB query → pool exhausted → all users affected | Script is throttled after N requests → other users unaffected           |
| Retry storm after partial failure                      | Client retries 1000x in 1 second → amplifies the failure               | Retries are rejected with 429 + `Retry-After` header → client backs off |
| Cost amplification (cloud DB)                          | Unbounded queries → unbounded compute cost                             | Rate cap bounds the maximum cost per time window                        |

Even for internal tooling, rate limiting prevents accidental self-inflicted outages.

## How — Implementation

### Architecture

A fixed-window rate limiter using `AtomicU64` for lock-free counter increments. A background `tokio` task resets the counter at the start of each window (every second).

**Files:**

- `src/rate_limiter.rs` — `RateLimiter` struct and background reset task
- `src/middleware.rs` — middleware functions that check the limiter

### Two tiers

| Tier   | Default limit | Env var                | Applies to                                                                                                          |
| ------ | ------------- | ---------------------- | ------------------------------------------------------------------------------------------------------------------- |
| Global | 100 req/s     | `RATE_LIMIT_RPS`       | All routes                                                                                                          |
| Heavy  | 10 req/s      | `RATE_LIMIT_HEAVY_RPS` | `/data/{table_name}`, `/column_values/{table_name}/{column_name}`, `/value_distribution/{table_name}/{column_name}` |

### Rejection response

When a client exceeds the limit, the middleware returns:

```
HTTP/1.1 429 Too Many Requests
Retry-After: 1
Content-Type: application/json

{"detail": "Rate limit exceeded"}
```

The `Retry-After: 1` header tells clients to wait one second before retrying (aligned with the fixed-window reset interval).

### Why fixed-window?

Fixed-window is the simplest rate limiting algorithm and has effectively zero overhead (one atomic increment per request, one background task per second). It can allow a brief burst at the window boundary (up to 2x the limit in a 1-second span across two windows), but for internal tooling this trade-off is acceptable. If stricter fairness is needed, the implementation can be swapped to a sliding window or token bucket without changing the middleware interface.

### Metrics

- `http_rate_limited_total` — counter incremented each time a request is rejected with 429
