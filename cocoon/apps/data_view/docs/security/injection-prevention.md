# Injection Prevention

> Status: **Implemented** — `src/routes/data_view/validation.rs`

## What

Preventing untrusted input from being interpreted as executable code (SQL, shell commands, HTML/JS). The primary vector in this service is SQL injection through table names, column names, and filter values.

## Why

SQL injection is consistently ranked #1 or #3 in the OWASP Top 10. It allows an attacker to:

| Attack               | Payload                                | Impact                      |
| -------------------- | -------------------------------------- | --------------------------- |
| Data exfiltration    | `' UNION SELECT password FROM users--` | All user credentials stolen |
| Data destruction     | `'; DROP TABLE orders;--`              | Permanent data loss         |
| Privilege escalation | `'; GRANT ALL TO attacker;--`          | Full database control       |
| Denial of service    | `'; SELECT pg_sleep(3600);--`          | DB resources exhausted      |

Even for internal tools, injection prevention is critical because:

- Internal users can make mistakes (pasting malformed data into a URL)
- Internal networks are not immune to compromised machines
- Audit and compliance requirements apply regardless of audience

## How — This Repo

### Whitelist validation

```rust
static TABLE_NAME_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[a-zA-Z_][a-zA-Z0-9_.]*$").unwrap());
```

Only characters valid in SQL identifiers are permitted. This is a **whitelist** — anything not explicitly allowed is rejected. Whitelists are fundamentally safer than blacklists because:

- Blacklist: "block `DROP`, `UNION`, `--`, `;`..." (attacker finds encoding you missed)
- Whitelist: "allow `[a-zA-Z0-9_.]`" (anything outside this set is rejected regardless of intent)

### Defence in depth

Multiple layers prevent injection even if one fails:

1. **Input validation** (first line): regex whitelist rejects invalid identifiers
2. **Parameterised queries** (where possible): bind parameters are escaped by the database driver
3. **Least privilege**: database user has only SELECT permissions where applicable
4. **Error handling**: database errors are logged but not returned verbatim to the client (prevents information leakage about DB structure)

### What NOT to do

```rust
// NEVER: string interpolation into SQL
let sql = format!("SELECT * FROM {table_name}");  // vulnerable

// INSTEAD: validate first, then use
validate_table_name(&table_name)?;
let sql = format!("SELECT * FROM {table_name}");  // safe — table_name is validated
```

Note: SQL identifiers (table/column names) cannot be parameterised in most drivers — they must be interpolated. This is why whitelist validation is essential for identifiers.
