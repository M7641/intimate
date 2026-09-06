# Circuit Breakers

> Status: **Implemented**

## What

A pattern that monitors failure rates for a downstream dependency (e.g. the database) and "trips" after consecutive failures, immediately rejecting new requests instead of forwarding them to the failing dependency. After a cooldown period, it allows a probe request through to check if the dependency has recovered.

The states:

```
CLOSED (normal) → failures exceed threshold → OPEN (reject all)
     ↑                                            │
     │              cooldown expires               ▼
     └─────────── HALF-OPEN (allow one probe) ─────┘
```

## Why

Without a circuit breaker, when the database goes down:

| Phase                | Without circuit breaker                                         | With circuit breaker                                             |
| -------------------- | --------------------------------------------------------------- | ---------------------------------------------------------------- |
| DB goes down         | Every request waits for `connection_timeout` (30s)              | After N failures, requests fail immediately (1ms)                |
| 100 concurrent users | 100 threads blocked for 30s each → pool + thread exhaustion     | Only the probe request waits; 99 get instant 503                 |
| DB recovers          | Thundering herd — all queued requests hit the DB simultaneously | Single probe succeeds → circuit closes → traffic ramps gradually |
| User experience      | 30-second hangs followed by errors                              | Instant "service unavailable" → users know to wait, not retry    |

Circuit breakers are **fail-fast** mechanisms. They protect both the service (from resource exhaustion) and the database (from thundering herd recovery).

## How — Implementation

### Architecture

A lock-free circuit breaker implemented with `std::sync::atomic` primitives (`AtomicU32` for state, `AtomicU64` for timestamps and counters). No mutex contention — safe to call from any async context.

**File:** `src/circuit_breaker.rs`

### State machine

```
CLOSED (0) ─── failures >= threshold ──→ OPEN (1)
   ↑                                        │
   │            cooldown expires             ▼
   └──────── HALF_OPEN (2) ─── probe succeeds
                  │
                  └── probe fails ──→ OPEN (1)
```

- **CLOSED:** Normal operation. Consecutive failures are tracked.
- **OPEN:** All requests are immediately rejected with `503 Service Unavailable`.
- **HALF_OPEN:** A single probe request is allowed through. Success closes the circuit; failure re-opens it.

### Configuration

| Env var                | Default | Description                                           |
| ---------------------- | ------- | ----------------------------------------------------- |
| `CB_FAILURE_THRESHOLD` | 5       | Consecutive failures before the circuit opens         |
| `CB_COOLDOWN_SECS`     | 30      | Seconds to wait in OPEN state before allowing a probe |

### Integration

The circuit breaker is integrated into `blocking_query()` in `src/state.rs`. Every database query passes through it:

1. **Before query:** check circuit state. If OPEN and cooldown has not elapsed, return `503` immediately.
2. **After query:** report success or failure to the breaker. Consecutive successes in HALF_OPEN close the circuit; failures re-open it.

### Metrics

| Metric                           | Type    | Description                                                 |
| -------------------------------- | ------- | ----------------------------------------------------------- |
| `db_circuit_breaker_state`       | Gauge   | Current state: 0 = closed, 1 = open, 2 = half-open          |
| `db_circuit_breaker_trips_total` | Counter | Number of times the circuit has tripped from closed to open |
