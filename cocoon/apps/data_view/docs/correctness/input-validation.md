# Input Validation

## What

Validating and sanitising all user-supplied input before it reaches the database or internal logic. This includes path parameters, query parameters, and any data used in SQL construction.

## Why

User input is the primary attack surface of any web service. Without validation:

| Attack              | Example                                    | Consequence                                        |
| ------------------- | ------------------------------------------ | -------------------------------------------------- |
| SQL injection       | `table_name = "users; DROP TABLE users--"` | Data loss, data exfiltration, privilege escalation |
| Resource exhaustion | `limit = 999999999`                        | OOM, DB timeout, denial of service                 |
| Path traversal      | `table_name = "../../../etc/passwd"`       | Information disclosure                             |
| Malformed input     | `column = ""` (empty string)               | Cryptic DB errors instead of clear 400 responses   |

Even for internal tools, input validation prevents accidental damage from typos and malformed URLs. A developer pasting a wrong table name into the browser shouldn't be able to cause harm.

## How

### Table name validation

```rust
// Only allows alphanumeric, underscores, dots, and schema-qualified names
static TABLE_NAME_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[a-zA-Z_][a-zA-Z0-9_.]*$").unwrap());

pub fn validate_table_name(name: &str) -> Result<(), AppError> {
    if !TABLE_NAME_RE.is_match(name) {
        return Err(AppError::Validation(
            format!("Invalid table name: {name}")
        ));
    }
    Ok(())
}
```

This whitelist approach is safer than blacklisting SQL keywords — it only permits characters that are valid in identifier names.

### Key principles

1. **Validate at the boundary**: check input in the handler, before passing to business logic or DB queries. Internal code can assume valid data.
2. **Whitelist, don't blacklist**: define what IS allowed rather than trying to enumerate everything that ISN'T. Attackers are creative; your blacklist isn't.
3. **Return 400 with detail**: tell the client exactly what's wrong so they can fix it without guessing.
4. **Validate types, ranges, and formats**: a `limit` should be a positive integer within a reasonable range, not just "any number".

### Error response

When validation fails, the client receives:

```json
{
  "detail": "Invalid table name: users; DROP TABLE users--"
}
```

Status: `400 Bad Request`. Log level: `warn` (not `error` — this is the client's fault).
