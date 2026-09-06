# Connection Pooling

> Status: **Implemented** — `src/pool.rs`, `src/state.rs`

## What

An `r2d2` connection pool manages a set of reusable database connections. Handlers borrow a connection from the pool, execute a query, and return it — without paying the cost of establishing a new TCP connection for each request.

## Why

| Without pooling                                                        | With pooling                                                                                        |
| ---------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------- |
| Each request opens a TCP connection + TLS handshake + auth (~50-200ms) | Connections are pre-established; checkout takes <1ms                                                |
| 100 concurrent requests = 100 simultaneous connections                 | 100 concurrent requests share 10 connections, queuing as needed                                     |
| Database overwhelmed by connection storms on traffic spikes            | Pool caps connections at `max_size`; excess requests wait in a bounded queue                        |
| No idle connection management — stale connections cause query failures | `idle_timeout` and `max_lifetime` evict stale connections; `test_on_check_out` validates before use |

Without pooling, every request to this data exploration API would add 50-200ms of connection overhead — often more than the query itself.

## How — This Repo

### Pool configuration (`src/pool.rs`)

```rust
r2d2::Pool::builder()
    .max_size(max_connections)              // DB_MAX_CONNECTIONS (default 10)
    .connection_timeout(Duration::from_secs(connection_timeout)) // DB_CONNECTION_TIMEOUT_SECS (default 30)
    .idle_timeout(Some(Duration::from_secs(idle_timeout)))       // DB_IDLE_TIMEOUT_SECS (default 600)
    .max_lifetime(Some(Duration::from_secs(max_lifetime)))       // DB_MAX_LIFETIME_SECS (default 1800)
    .test_on_check_out(true)               // Validate connection before use
    .build(manager)
```

| Parameter            | Default | Purpose                                                        |
| -------------------- | ------- | -------------------------------------------------------------- |
| `max_size`           | 10      | Maximum concurrent connections — limits DB load                |
| `connection_timeout` | 30s     | How long to wait for a free connection before failing          |
| `idle_timeout`       | 600s    | Evict connections idle for 10 minutes                          |
| `max_lifetime`       | 1800s   | Evict connections older than 30 minutes (prevents stale state) |
| `test_on_check_out`  | true    | Ping the connection before handing it to a handler             |

### Pool metrics (`src/state.rs`)

```rust
let ps = self.pool.state();
metrics::gauge!("db_pool_connections_total").set(f64::from(ps.connections));
metrics::gauge!("db_pool_connections_idle").set(f64::from(ps.idle_connections));
```

Recorded on every query — gives Prometheus a continuous view of pool utilisation.

### Readiness check (`src/routes/health.rs`)

```rust
"pool": {
    "active": pool.connections - pool.idle_connections,
    "idle": pool.idle_connections,
    "total": pool.connections,
}
```

The readiness probe reports pool stats so operators can spot exhaustion before it causes timeouts.

### Sizing guidance

- **Start with `max_size` = 2-3x your expected concurrency.** For an internal tool with ~5 concurrent users, 10 is generous.
- **Watch `db_pool_connections_idle`:** if it's consistently at 0, increase `max_size`. If it's consistently close to `max_size`, you're over-provisioned.
- **`connection_timeout` should be short enough to fail fast** (30s is a reasonable upper bound) but not so short that transient DB pauses cause cascading failures.
