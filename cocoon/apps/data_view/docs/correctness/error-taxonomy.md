# Error Taxonomy

## What

A structured categorisation of every error the service can produce, mapped to HTTP status codes, log levels, and machine-parseable fields.

## Why

Most services start with a single error type: "something went wrong, here's a 500". This fails in production for three reasons:

| Problem                             | Impact                                                                                                                                                        |
| ----------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **All errors return 500**           | Clients can't distinguish "your input is wrong" (fix it) from "our server is broken" (wait and retry). They retry everything, amplifying load during outages. |
| **All errors logged at `error!`**   | On-call gets paged for client typos. Alert fatigue sets in. Real errors get ignored.                                                                          |
| **Error messages expose internals** | `"ODBC error: connection refused to host 10.0.3.42:5439"` leaks infrastructure details.                                                                       |

A proper taxonomy means:

- Clients get **actionable HTTP status codes** (400 = fix your request; 404 = check the URL; 500 = not your fault, retry later)
- Operations gets **appropriate alert severity** (warn for client errors; error for server failures)
- Log queries can **filter by error class** (`error.kind="database"` → only DB failures)

## How — This Repo

### Error variants (`src/error.rs`)

```rust
pub enum AppError {
    Validation(String),      // 400 — client sent bad input
    NotFound(String),        // 404 — resource doesn't exist
    Database(DatabaseError), // 500 — DB layer failure
    Internal(String),        // 500 — unexpected server error
}
```

### HTTP mapping

| Variant      | HTTP Status               | Log Level | Rationale                                   |
| ------------ | ------------------------- | --------- | ------------------------------------------- |
| `Validation` | 400 Bad Request           | `warn`    | Client error — expected in normal operation |
| `NotFound`   | 404 Not Found             | `warn`    | Client error — wrong URL or stale link      |
| `Database`   | 500 Internal Server Error | `error`   | Server failure — needs investigation        |
| `Internal`   | 500 Internal Server Error | `error`   | Server failure — needs investigation        |

### Structured logging in the error handler

```rust
AppError::Database(err) => {
    tracing::error!(error.kind = "database", error = %err, "Database error");
    (StatusCode::INTERNAL_SERVER_ERROR, err.to_string())
}
```

The `error.kind` field allows filtering in log aggregation:

```
error.kind="database"    → DB issues (check connectivity, query plans)
error.kind="internal"    → Code bugs (check stack traces, recent deployments)
error.kind="validation"  → Client errors (check API docs, client code)
error.kind="not_found"   → Missing resources (check URLs, data availability)
```

### Why `warn` for 4xx, `error` for 5xx?

- **4xx errors** are the client's problem. Alerting on them creates noise (bots, typos, expired links). Monitor their rate via metrics, but don't page on individual occurrences.
- **5xx errors** are our fault. Every single one deserves investigation. Alert on `error`-level logs and on `http_requests_total{status=~"5.."}`.

### Automatic conversion

```rust
impl From<DatabaseError> for AppError {
    fn from(err: DatabaseError) -> Self {
        AppError::Database(err)
    }
}
```

The `?` operator in handlers automatically converts database errors into the correct error variant with proper logging and status code. No manual error mapping in business logic.
