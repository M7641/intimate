# CORS & Security Headers

> Status: **Implemented**

## What

**CORS** (Cross-Origin Resource Sharing) controls which web origins can make requests to the API from a browser. **Security headers** instruct browsers to enable protections against common attacks.

## Why

| Without CORS policy                                                                  | With CORS policy                                                                |
| ------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------- |
| Any website can make API calls from a user's browser using their cookies/credentials | Only allowed origins (e.g. your frontend domain) can make cross-origin requests |
| A malicious page could exfiltrate data via `fetch()`                                 | Browser blocks the response before JavaScript can read it                       |

| Without security headers                              | With security headers                                     |
| ----------------------------------------------------- | --------------------------------------------------------- |
| Browser guesses content type → MIME confusion attacks | `X-Content-Type-Options: nosniff` enforces declared types |
| No transport security enforcement                     | `Strict-Transport-Security` forces HTTPS                  |

> **Note on framing / `X-Frame-Options`:** this header is intentionally not set,
> so responses can be embedded in an iframe from any origin. The header only
> accepts `DENY`/`SAMEORIGIN` (its `ALLOW-FROM` form is obsolete), so there is no
> "allow any origin" value — its presence would block cross-origin framing. The
> trade-off is that there is no built-in clickjacking protection; to restrict
> framing later, send a `Content-Security-Policy: frame-ancestors <list>` header
> instead.

Even for internal tools, CORS matters because internal networks host many web applications, and a compromised or malicious internal page could exploit the API.

## How — Implementation

### CORS

CORS is configured via `tower_http::CorsLayer` in `src/app.rs` (`build_cors_layer`).

**Configuration:**

| Setting         | Value                                                                              |
| --------------- | ---------------------------------------------------------------------------------- |
| Allowed origins | Env var `CORS_ALLOWED_ORIGINS` (default: `http://localhost:5173`), comma-separated |
| Allowed methods | GET, POST, PUT, DELETE, OPTIONS                                                    |
| Allowed headers | `content-type`, `authorization`, `x-request-id`                                    |
| Max age         | 3600 seconds                                                                       |

Multiple origins can be specified by separating them with commas:

```bash
CORS_ALLOWED_ORIGINS="https://app.example.com,https://staging.example.com"
```

### Security headers

A custom middleware in `src/middleware.rs` (`security_headers`) adds the following headers to every response:

| Header                   | Value                             | Purpose                                              |
| ------------------------ | --------------------------------- | ---------------------------------------------------- |
| `x-content-type-options` | `nosniff`                         | Prevents MIME-type sniffing                          |
| `x-xss-protection`       | `0`                               | Disables legacy XSS filter (modern CSP is preferred) |
| `referrer-policy`        | `strict-origin-when-cross-origin` | Limits referrer leakage on cross-origin requests     |

### Files

- `src/app.rs` — `build_cors_layer` function
- `src/middleware.rs` — `security_headers` middleware
