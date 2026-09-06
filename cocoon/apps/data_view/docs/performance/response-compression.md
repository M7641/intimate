# Response Compression

> Status: **Implemented** — `src/app.rs` via `CompressionLayer`

## What

Automatic gzip compression of HTTP response bodies. The server checks the `Accept-Encoding` request header and compresses the response if the client supports it.

## Why

This service returns JSON — often large JSON arrays from table scans, column distributions, and warehouse analytics. JSON compresses extremely well (typically 5-10x reduction).

| Endpoint | Uncompressed | Gzip compressed | Reduction |
|----------|-------------|-----------------|-----------|
| `/api/data_view/data/{table}` (1000 rows) | ~500 KB | ~50 KB | 90% |
| `/api/data_view/column_values/{table}/{col}` | ~100 KB | ~15 KB | 85% |
| `/api/warehouse/redshift/query-performance` | ~200 KB | ~25 KB | 87% |

The impact:

| Without compression | With compression |
|--------------------|-----------------|
| 500 KB per response × 100 req/s = 50 MB/s bandwidth | 50 KB × 100 req/s = 5 MB/s — 10x less |
| Slow page loads on remote/VPN connections | Responsive UI even on constrained networks |
| Higher cloud egress costs | Significantly lower transfer costs |

## How — This Repo

```rust
// src/app.rs
use tower_http::compression::CompressionLayer;

Router::new()
    // ... routes ...
    .layer(CompressionLayer::new())  // innermost layer
```

### Why innermost?

Compression operates on the response body after all other processing is complete. Placing it as the innermost layer means:

1. The metrics middleware measures **pre-compression** duration (true server time, not including compression overhead)
2. The TraceLayer logs **pre-compression** response size
3. Compression adds ~1-5ms per response — negligible compared to DB queries

### Client negotiation

The client must send `Accept-Encoding: gzip` (all modern browsers and HTTP clients do). If absent, the response is sent uncompressed — no compatibility risk.

```
Request:  Accept-Encoding: gzip, deflate, br
Response: Content-Encoding: gzip
```
