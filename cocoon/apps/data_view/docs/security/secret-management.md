# Secret Management

> Status: **Partial** — secrets are read from environment variables; no vault integration

## What

How sensitive credentials (database passwords, API keys, tokens) are stored, accessed, and protected from accidental exposure.

## Why

| Bad practice                       | Risk                                                                          |
| ---------------------------------- | ----------------------------------------------------------------------------- |
| Secrets in source code             | Anyone with repo access sees credentials; they persist in git history forever |
| Secrets in docker images           | Image layers are immutable; credentials are extractable even after "deletion" |
| Secrets in logs                    | Log aggregation systems store them searchable and often unencrypted           |
| Secrets in error messages          | Response bodies visible to clients, proxies, and CDN caches                   |
| Shared secrets across environments | Dev credentials work in production; a dev mistake becomes a prod incident     |

A single leaked database password can grant full access to production data. Secret management is not paranoia — it's baseline hygiene.

## How — This Repo

### Current approach: environment variables

```rust
// src/pool.rs
let config = DatabaseConfig::from_env(warehouse_type)?;
```

Database credentials are read from environment variables at startup. This is acceptable for:

- Local development (`.env` files, not committed)
- Container orchestration (Kubernetes Secrets, ECS task definitions)
- CI/CD (GitHub Actions secrets, AWS Parameter Store)

### What the code does NOT do (correctly)

1. **No secrets in source code**: credentials are never hardcoded
2. **No secrets in logs**: `tracing::debug!(sql = %sql)` logs the SQL text but connection strings (which may contain passwords) are never logged
3. **No secrets in error responses**: `AppError::Database` logs the error server-side but returns a generic message to the client

### What could be improved

| Current                                    | Improvement                                                  |
| ------------------------------------------ | ------------------------------------------------------------ |
| Plain env vars                             | AWS Secrets Manager / HashiCorp Vault for rotation and audit |
| Credentials in memory for process lifetime | Short-lived tokens with automatic refresh                    |
| No credential rotation                     | Vault dynamic secrets — new credentials per deployment       |
| No audit trail for secret access           | Vault audit log shows who accessed what and when             |

### Environment variable checklist

| Variable                         | Contains secret? | Notes                                                  |
| -------------------------------- | ---------------- | ------------------------------------------------------ |
| `DATA_WAREHOUSE_TYPE`            | No               | Safe to log                                            |
| `RUST_LOG`                       | No               | Safe to log                                            |
| `RUST_ENV`                       | No               | Safe to log                                            |
| Database host/port/user/password | **Yes**          | Read by `DatabaseConfig::from_env()` — never log these |
| `DB_MAX_CONNECTIONS`             | No               | Safe to log                                            |

### Rule: never log connection strings

```rust
// WRONG
tracing::info!("Connecting to {}", connection_string);

// RIGHT
tracing::info!("Connecting to database (warehouse_type={warehouse_type})...");
```

The startup message in `main.rs` logs the warehouse type (safe) but not the connection details (secret).
