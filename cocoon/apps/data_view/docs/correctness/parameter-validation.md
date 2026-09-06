# Numeric Parameter Bounds

> Status: **Implemented**

## What

Server-side validation of numeric query parameters (`days`, `limit`, `top_n`, etc.) to prevent unreasonable values from reaching the database.

## Why

| Without bounds                                                            | With bounds                                 |
| ------------------------------------------------------------------------- | ------------------------------------------- |
| `?days=999999` → SQL scans years of data → warehouse timeout or huge cost | Rejected with 400: "days must be <= 365"    |
| `?limit=999999999` → server allocates massive Vec → OOM                   | Rejected with 400: "limit must be <= 10000" |

## How

### Data View endpoints (`src/routes/data_view/data.rs`)

```rust
const MAX_DATA_LIMIT: u32 = 10_000;

if params.limit > MAX_DATA_LIMIT {
    return Err(AppError::Validation(...));
}
```

### Warehouse endpoints (`src/routes/warehouse/{redshift,snowflake}/handlers.rs`)

```rust
const MAX_DAYS: u32 = 365;
const MAX_LIMIT: u32 = 1000;

fn validate_days(days: u32) -> Result<(), AppError> { ... }
fn validate_limit(limit: u32) -> Result<(), AppError> { ... }
```

Every handler that accepts `days` or `limit` calls the corresponding validator before building the SQL query.

## Bounds

| Parameter           | Max    | Rationale                                                      |
| ------------------- | ------ | -------------------------------------------------------------- |
| `days` (warehouse)  | 365    | Cloud warehouse query history is typically retained for 1 year |
| `limit` (warehouse) | 1,000  | Warehouse analytics queries return summary data                |
| `limit` (data view) | 10,000 | Full row data — larger sets should use export/pagination       |
