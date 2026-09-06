use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use serde_json::Value;

use crate::schema::{parse_table_name, schema_from_rows};
use crate::sql::{parse_text_value, substitute_params};
use crate::traits::{Database, DatabaseError, QueryResult, Row};

// -- Config --

/// Configuration for Snowflake REST API connections.
#[derive(Debug, Clone)]
pub struct SnowflakeConfig {
    /// Snowflake account identifier (e.g. "xy12345.eu-west-1").
    pub account: String,
    /// OAuth access token.
    pub oauth_token: String,
    /// Warehouse to use for queries.
    pub warehouse: Option<String>,
    /// Database name.
    pub database: Option<String>,
    /// Schema name.
    pub schema: Option<String>,
    /// Role to use.
    pub role: Option<String>,
    /// Base URL for the SQL API, without the `/api/v2/statements` suffix.
    /// Defaults to `https://{account}.snowflakecomputing.com`; override via
    /// `SNOWFLAKE_BASE_URL` to point at a local mock in tests.
    pub base_url: String,
    /// Server-side statement timeout (the SQL API `timeout` field, in seconds).
    /// `None` keeps the legacy 60s default. See
    /// [`DatabaseConfig::with_statement_timeout`](crate::DatabaseConfig::with_statement_timeout).
    pub statement_timeout: Option<Duration>,
    /// JSON query tag applied to every statement (Snowflake's `QUERY_TAG`
    /// session parameter), so each query is attributable in QUERY_HISTORY for
    /// cost analysis. Built by [`build_query_tag`].
    pub query_tag: Option<String>,
}

/// Derive the default API base URL from a Snowflake account identifier.
fn default_base_url(account: &str) -> String {
    format!("https://{account}.snowflakecomputing.com")
}

/// Build the JSON query tag applied to every statement.
///
/// `SNOWFLAKE_QUERY_TAG` overrides the whole tag (raw JSON, for full control).
/// Otherwise a Nimbus-convention tag is assembled from the application name
/// (`SNOWFLAKE_QUERY_TAG_SOURCE`, defaulting to `snowhouse_application`) and the runtime
/// environment (`RUST_ENV`), following `{"nimbus@source": ..., "nimbus@owner":
/// "system", "nimbus@env": ...}`. Every query then carries its origin into
/// Snowflake's `QUERY_HISTORY.QUERY_TAG`, which is the basis for per-application
/// cost attribution.
fn build_query_tag() -> Option<String> {
    if let Ok(tag) = std::env::var("SNOWFLAKE_QUERY_TAG") {
        return Some(tag);
    }
    let source = std::env::var("SNOWFLAKE_QUERY_TAG_SOURCE")
        .unwrap_or_else(|_| "snowhouse_application".to_string());
    let env = std::env::var("RUST_ENV").unwrap_or_else(|_| "development".to_string());
    Some(compose_query_tag(&source, &env))
}

/// Assemble the Nimbus-convention query tag JSON from its parts. Kept pure (no env
/// reads) so it is testable; `serde_json` escapes any special characters.
fn compose_query_tag(source: &str, env: &str) -> String {
    serde_json::json!({
        "nimbus@source": source,
        "nimbus@owner": "system",
        "nimbus@env": env,
    })
    .to_string()
}

/// Credits-per-hour a Snowflake warehouse burns while running, keyed by its
/// size. Each step up the scale doubles the rate, so the gap between an X-Small
/// (1) and a Large (8) is exactly what makes an unnoticed oversized warehouse
/// expensive. Returns `None` for an unrecognised size (e.g. one Snowflake adds
/// later), so the caller can still log the raw name. Kept pure for testing.
fn credits_per_hour(size: &str) -> Option<f64> {
    match size.trim().to_uppercase().as_str() {
        "X-SMALL" | "XSMALL" => Some(1.0),
        "SMALL" => Some(2.0),
        "MEDIUM" => Some(4.0),
        "LARGE" => Some(8.0),
        "X-LARGE" | "XLARGE" => Some(16.0),
        "2X-LARGE" | "X2LARGE" => Some(32.0),
        "3X-LARGE" | "X3LARGE" => Some(64.0),
        "4X-LARGE" | "X4LARGE" => Some(128.0),
        "5X-LARGE" | "X5LARGE" => Some(256.0),
        "6X-LARGE" | "X6LARGE" => Some(512.0),
        _ => None,
    }
}

/// Credit rate (Large, 8/hour) at or above which the warehouse size is worth
/// flagging as a warning rather than plain info.
const LARGE_WAREHOUSE_CREDITS: f64 = 8.0;

/// Process-wide switch for smallest-warehouse auto-selection. Off by default, so
/// the shared connector is unchanged for other apps (e.g. `data_view`, `tako`);
/// an app that wants it — like snowhouse — turns it on at startup via
/// [`enable_smallest_warehouse`].
static AUTO_SMALLEST_WAREHOUSE: AtomicBool = AtomicBool::new(false);

/// Make every Snowflake connection run its statements on the smallest warehouse
/// the role can see (the cheapest credit rate), resolved once via
/// `SHOW WAREHOUSES` and cached for the process.
///
/// Call once at startup, before opening connections. Intended for read-only
/// analytics apps that never need compute: it overrides both `SNOWFLAKE_WAREHOUSE`
/// and the warehouse from the Nimbus connection. Idempotent.
pub fn enable_smallest_warehouse() {
    AUTO_SMALLEST_WAREHOUSE.store(true, Ordering::Relaxed);
}

/// Choose the cheapest warehouse from `(name, size)` pairs, by credit rate.
///
/// Unknown sizes are ranked last (treated as `+inf`) so an unrecognised — and
/// possibly large — size is never mistaken for the smallest. Ties break on name
/// so the pick is stable and predictable across restarts. Pure, for testing.
fn cheapest_warehouse<'a>(
    candidates: impl IntoIterator<Item = (&'a str, &'a str)>,
) -> Option<String> {
    let mut best: Option<(f64, &str)> = None;
    for (name, size) in candidates {
        let rank = credits_per_hour(size).unwrap_or(f64::INFINITY);
        let is_better = match best {
            None => true,
            Some((best_rank, best_name)) => {
                rank < best_rank || (rank == best_rank && name < best_name)
            }
        };
        if is_better {
            best = Some((rank, name));
        }
    }
    best.map(|(_, name)| name.to_string())
}

/// Where the Snowflake OAuth token comes from (see [`SnowflakeConfig::resolve`]).
#[derive(Debug, PartialEq, Eq)]
enum AuthMode {
    /// Fetch an OAuth token from the Nimbus connections API (`API_KEY`). Default.
    OAuth,
    /// Read `SNOWFLAKE_ACCOUNT` / `SNOWFLAKE_OAUTH_TOKEN` from the environment.
    Env,
}

impl AuthMode {
    /// Parse the `SNOWFLAKE_AUTH_TYPE` value (case- and whitespace-insensitive).
    fn parse(auth: &str) -> Result<Self, DatabaseError> {
        match auth.trim().to_lowercase().as_str() {
            "oauth" => Ok(Self::OAuth),
            "env" => Ok(Self::Env),
            other => Err(DatabaseError::ConnectionError(format!(
                "Unknown SNOWFLAKE_AUTH_TYPE '{other}' (expected 'oauth' or 'env')"
            ))),
        }
    }
}

impl SnowflakeConfig {
    /// Build configuration from environment variables.
    ///
    /// Uses: `SNOWFLAKE_ACCOUNT`, `SNOWFLAKE_OAUTH_TOKEN`, `SNOWFLAKE_WAREHOUSE`,
    /// `SNOWFLAKE_DATABASE`, `SNOWFLAKE_SCHEMA`, `SNOWFLAKE_ROLE`.
    pub fn from_env() -> Self {
        let account = std::env::var("SNOWFLAKE_ACCOUNT").unwrap_or_default();
        let base_url =
            std::env::var("SNOWFLAKE_BASE_URL").unwrap_or_else(|_| default_base_url(&account));
        Self {
            oauth_token: std::env::var("SNOWFLAKE_OAUTH_TOKEN").unwrap_or_default(),
            warehouse: std::env::var("SNOWFLAKE_WAREHOUSE").ok(),
            database: std::env::var("SNOWFLAKE_DATABASE").ok(),
            schema: std::env::var("SNOWFLAKE_SCHEMA").ok(),
            role: std::env::var("SNOWFLAKE_ROLE").ok(),
            account,
            base_url,
            // Off by default (keeps the legacy 60s); callers opt in.
            statement_timeout: None,
            query_tag: build_query_tag(),
        }
    }

    /// Build configuration from Nimbus OAuth credentials.
    ///
    /// Fetches credentials from the Nimbus connections API (requires `API_KEY` env var),
    /// then constructs a SnowflakeConfig with the returned values.
    pub fn from_nimbus_api() -> Result<Self, DatabaseError> {
        use std::collections::HashMap;

        const NIMBUS_CREDENTIALS_URL: &str =
            "https://service.nimbus.example/connections/api/v1/connections/credentials";

        tracing::debug!(
            url = NIMBUS_CREDENTIALS_URL,
            "Fetching Snowflake credentials from Nimbus"
        );

        let api_key = std::env::var("API_KEY").map_err(|_| {
            tracing::error!(
                "API_KEY not set — required when SNOWFLAKE_AUTH_TYPE=oauth (default). \
                 Set API_KEY, or set SNOWFLAKE_AUTH_TYPE=env for direct credentials."
            );
            DatabaseError::ConnectionError("API_KEY env var not set".into())
        })?;

        let response = reqwest::blocking::Client::new()
            .get(NIMBUS_CREDENTIALS_URL)
            .header("Authorization", &api_key)
            .timeout(Duration::from_secs(10))
            .send()
            .map_err(|e| {
                tracing::error!(error = %e, "Nimbus credentials request failed (network/timeout)");
                DatabaseError::ConnectionError(format!("Nimbus API request failed: {e}"))
            })?;

        let status = response.status();
        let body = response.text().map_err(|e| {
            DatabaseError::ConnectionError(format!("Failed to read response body: {e}"))
        })?;

        if !status.is_success() {
            // Body here is an error payload (not the token), safe to log.
            tracing::error!(%status, body = %body, "Nimbus credentials endpoint returned non-success");
            return Err(DatabaseError::ConnectionError(format!(
                "Nimbus API returned {status}: {body}"
            )));
        }

        let creds: HashMap<String, Value> = serde_json::from_str(&body).map_err(|e| {
            DatabaseError::ConnectionError(format!("Nimbus API response parse failed: {e}"))
        })?;

        let get = |key: &str| match creds.get(key) {
            Some(Value::String(s)) => s.clone(),
            Some(v) => v.to_string(),
            None => String::new(),
        };
        let get_opt = |key: &str| match creds.get(key) {
            Some(Value::String(s)) if !s.is_empty() => Some(s.clone()),
            _ => None,
        };

        let account = get("host");
        let base_url =
            std::env::var("SNOWFLAKE_BASE_URL").unwrap_or_else(|_| default_base_url(&account));
        let cfg = Self {
            account,
            base_url,
            oauth_token: get("accessToken"),
            warehouse: get_opt("warehouse"),
            database: get_opt("database"),
            schema: get_opt("schema"),
            role: None,
            statement_timeout: None,
            query_tag: build_query_tag(),
        };

        // Never log the token itself — only whether it is present, and its length.
        tracing::debug!(
            account = %cfg.account,
            warehouse = ?cfg.warehouse,
            database = ?cfg.database,
            schema = ?cfg.schema,
            token_present = !cfg.oauth_token.is_empty(),
            token_len = cfg.oauth_token.len(),
            "Received Snowflake credentials from Nimbus"
        );
        if cfg.account.is_empty() || cfg.oauth_token.is_empty() {
            tracing::warn!(
                account_present = !cfg.account.is_empty(),
                token_present = !cfg.oauth_token.is_empty(),
                "Nimbus response is missing `host` and/or `accessToken` — check the \
                 connection is a Snowflake OAuth connection and API_KEY has access"
            );
        }
        Ok(cfg)
    }

    /// Resolve the config from the configured auth source.
    ///
    /// Selected by `SNOWFLAKE_AUTH_TYPE`, defaulting to `oauth`:
    /// - `oauth` → fetch the token from the Nimbus connections API using `API_KEY`
    ///   ([`from_nimbus_api`](Self::from_nimbus_api)).
    /// - `env` → read `SNOWFLAKE_ACCOUNT` / `SNOWFLAKE_OAUTH_TOKEN` directly
    ///   ([`from_env`](Self::from_env)), handy for local testing.
    pub fn resolve() -> Result<Self, DatabaseError> {
        let auth = std::env::var("SNOWFLAKE_AUTH_TYPE").unwrap_or_else(|_| "oauth".to_string());
        tracing::debug!(auth_type = %auth, "Resolving Snowflake auth source");
        match AuthMode::parse(&auth)? {
            AuthMode::OAuth => Self::from_nimbus_api(),
            AuthMode::Env => Ok(Self::from_env()),
        }
    }

    /// Build the base URL for the Snowflake SQL API.
    fn api_url(&self) -> String {
        format!("{}/api/v2/statements", self.base_url)
    }
}

// -- Database implementation --

/// Snowflake database connector using the REST API v2.
///
/// No system dependencies — uses `reqwest::blocking::Client` for HTTP.
pub struct SnowflakeDatabase {
    client: reqwest::blocking::Client,
    config: SnowflakeConfig,
    default_schema: String,
    /// Live OAuth token. Kept behind a lock so it can be refreshed in place when
    /// Snowflake rejects it (401) — Nimbus tokens are short-lived (~10 min).
    token: std::sync::RwLock<String>,
}

/// Maximum number of poll attempts for async statements (202 responses).
const MAX_POLL_ATTEMPTS: u32 = 60;

/// Initial backoff delay in milliseconds for polling.
const INITIAL_BACKOFF_MS: u64 = 500;

/// Sent on every SQL API request. Snowflake's REST API rejects requests with an
/// empty/missing User-Agent ("Invalid or empty User-Agent header set: null"),
/// and reqwest sends none by default, so we set one explicitly.
const SNOWFLAKE_USER_AGENT: &str = concat!("cocoon-database/", env!("CARGO_PKG_VERSION"));

/// Process-once guard for the warehouse-size log. The pool opens a fresh
/// connection per request, so without this every connect would re-run
/// `SHOW WAREHOUSES`. We only want the size announced once, at startup.
static WAREHOUSE_SIZE_LOGGED: std::sync::Once = std::sync::Once::new();

/// Process-wide cache of the auto-selected warehouse (see [`WAREHOUSE_AUTO_ENV`]).
/// The pool opens a fresh connection per request, so this keeps the SHOW
/// WAREHOUSES pick to once per process. `None` records "auto was on but nothing
/// could be chosen", so we don't retry the lookup on every connection.
static AUTO_WAREHOUSE: OnceLock<Option<String>> = OnceLock::new();

impl SnowflakeDatabase {
    /// Create a new Snowflake connection from environment variables.
    pub fn connect() -> Result<Self, DatabaseError> {
        Self::connect_with_config(SnowflakeConfig::from_env())
    }

    /// Create a new Snowflake connection with explicit configuration.
    pub fn connect_with_config(config: SnowflakeConfig) -> Result<Self, DatabaseError> {
        tracing::debug!(
            account = %config.account,
            base_url = %config.base_url,
            warehouse = ?config.warehouse,
            database = ?config.database,
            "Opening Snowflake connection"
        );
        if config.account.is_empty() || config.oauth_token.is_empty() {
            tracing::error!(
                account_empty = config.account.is_empty(),
                token_empty = config.oauth_token.is_empty(),
                "Cannot connect: Snowflake account and/or oauth_token are empty"
            );
            return Err(DatabaseError::ConnectionError(
                "Snowflake account and oauth_token are required".into(),
            ));
        }

        let default_schema = config
            .schema
            .clone()
            .unwrap_or_else(|| "PUBLIC".to_string());

        // Transport timeout for the SQL API itself, matched to the server-side
        // statement timeout so every layer agrees on one number. Derived from
        // `statement_timeout` (default 60s) so it tracks STATEMENT_TIMEOUT_SECS.
        let http_timeout = config
            .statement_timeout
            .unwrap_or_else(|| Duration::from_secs(60));
        let client = reqwest::blocking::Client::builder()
            .timeout(http_timeout)
            .user_agent(SNOWFLAKE_USER_AGENT)
            .build()
            .map_err(|e| DatabaseError::ConnectionError(format!("HTTP client error: {e}")))?;

        let token = std::sync::RwLock::new(config.oauth_token.clone());
        let mut db = Self {
            client,
            config,
            default_schema,
            token,
        };
        // If auto-selection is on, repoint this connection at the smallest
        // warehouse the role can see (resolved once per process). Do this before
        // the size log so it reports the warehouse actually in use.
        db.apply_auto_warehouse();
        // Announce the warehouse size once, so an oversized (credit-hungry)
        // warehouse is caught at startup rather than on the monthly bill.
        db.log_warehouse_size_once();
        Ok(db)
    }

    /// Log the configured warehouse's size exactly once per process.
    ///
    /// The size (not the name) sets the credit burn rate, so surfacing it makes
    /// an accidental Large/X-Large obvious. Best-effort and non-fatal: a missing
    /// size or a failed lookup is logged at debug and the connection proceeds.
    fn log_warehouse_size_once(&self) {
        let Some(warehouse) = self.config.warehouse.clone() else {
            return;
        };
        WAREHOUSE_SIZE_LOGGED.call_once(|| match self.warehouse_size(&warehouse) {
            Ok(Some(size)) => match credits_per_hour(&size) {
                Some(credits) if credits >= LARGE_WAREHOUSE_CREDITS => tracing::warn!(
                    warehouse = %warehouse,
                    size = %size,
                    credits_per_hour = credits,
                    "Snowflake warehouse is large — {credits} credits/hour while running; \
                     confirm this size is intended to avoid wasting credits"
                ),
                Some(credits) => tracing::info!(
                    warehouse = %warehouse,
                    size = %size,
                    credits_per_hour = credits,
                    "Snowflake warehouse size"
                ),
                None => tracing::info!(
                    warehouse = %warehouse,
                    size = %size,
                    "Snowflake warehouse size (unrecognised — credit rate unknown)"
                ),
            },
            Ok(None) => {
                tracing::debug!(warehouse = %warehouse, "SHOW WAREHOUSES returned no size")
            }
            Err(e) => {
                tracing::debug!(warehouse = %warehouse, error = %e, "Warehouse size lookup failed")
            }
        });
    }

    /// Read the size of a warehouse (`X-Small` … `6X-Large`) via SHOW WAREHOUSES.
    ///
    /// SHOW runs on the services layer with no active warehouse, so it costs
    /// nothing. Returns `None` if the warehouse is not visible to the role.
    fn warehouse_size(&self, warehouse: &str) -> Result<Option<String>, DatabaseError> {
        let sql = format!("SHOW WAREHOUSES LIKE '{}'", warehouse.replace('\'', "''"));
        let rows = self.query(&sql, &[])?;
        Ok(rows
            .into_iter()
            .next()
            .and_then(|row| row.get("size").and_then(Value::as_str).map(str::to_string)))
    }

    /// When [`WAREHOUSE_AUTO_ENV`] is on, repoint the connection at the cheapest
    /// warehouse the role can see. Resolved once per process and cached, so only
    /// the first connection runs `SHOW WAREHOUSES`. A failed or empty lookup is
    /// non-fatal: the configured warehouse is left untouched as a fallback.
    fn apply_auto_warehouse(&mut self) {
        if !AUTO_SMALLEST_WAREHOUSE.load(Ordering::Relaxed) {
            return;
        }
        let picked = AUTO_WAREHOUSE
            .get_or_init(|| match self.smallest_warehouse() {
                Ok(Some(name)) => {
                    tracing::info!(
                        warehouse = %name,
                        "Auto-selected smallest Snowflake warehouse"
                    );
                    Some(name)
                }
                Ok(None) => {
                    tracing::warn!(
                        "Smallest-warehouse selection is on but SHOW WAREHOUSES returned none to \
                         rank — keeping the configured warehouse"
                    );
                    None
                }
                Err(e) => {
                    tracing::warn!(
                        error = %e,
                        "Smallest-warehouse selection is on but SHOW WAREHOUSES failed — \
                         keeping the configured warehouse"
                    );
                    None
                }
            })
            .clone();
        if let Some(name) = picked {
            self.config.warehouse = Some(name);
        }
    }

    /// Pick the cheapest warehouse the role can see, by credit rate.
    ///
    /// `SHOW WAREHOUSES` lists every warehouse visible to the role and its size;
    /// [`cheapest_warehouse`] ranks them. Runs on the services layer, so it costs
    /// no warehouse credits.
    ///
    /// Caveat: `SHOW WAREHOUSES` reflects visibility, not the `USAGE` privilege,
    /// so the pick is not guaranteed usable — an unusable one surfaces as a clear
    /// error on the first query rather than being silently skipped here.
    fn smallest_warehouse(&self) -> Result<Option<String>, DatabaseError> {
        let rows = self.query("SHOW WAREHOUSES", &[])?;
        let pairs: Vec<(String, String)> = rows
            .into_iter()
            .filter_map(|row| {
                let name = row.get("name").and_then(Value::as_str)?.to_string();
                let size = row.get("size").and_then(Value::as_str)?.to_string();
                Some((name, size))
            })
            .collect();
        Ok(cheapest_warehouse(
            pairs.iter().map(|(n, s)| (n.as_str(), s.as_str())),
        ))
    }

    /// The current OAuth token (refreshable via [`refresh_token`](Self::refresh_token)).
    fn current_token(&self) -> String {
        self.token
            .read()
            .expect("Snowflake token lock poisoned")
            .clone()
    }

    /// Re-authenticate and replace the live token in place.
    ///
    /// Respects `SNOWFLAKE_AUTH_TYPE` (Nimbus OAuth by default), so on Nimbus this
    /// fetches a fresh short-lived token. Called when Snowflake returns 401.
    fn refresh_token(&self) -> Result<(), DatabaseError> {
        let fresh = SnowflakeConfig::resolve()?;
        if fresh.oauth_token.is_empty() {
            return Err(DatabaseError::ConnectionError(
                "Re-authentication returned an empty token".into(),
            ));
        }
        *self.token.write().expect("Snowflake token lock poisoned") = fresh.oauth_token;
        tracing::debug!("Snowflake OAuth token refreshed");
        Ok(())
    }

    /// Submit a SQL statement to the Snowflake REST API.
    ///
    /// Returns the full JSON response body. Handles async (202) responses
    /// by polling until the statement completes.
    fn submit_statement(&self, sql: &str) -> Result<Value, DatabaseError> {
        let url = self.config.api_url();

        // Build request body. The `timeout` field is Snowflake's server-side
        // statement timeout (seconds); fall back to the legacy 60s when unset.
        let timeout_secs = self
            .config
            .statement_timeout
            .map(|d| d.as_secs())
            .unwrap_or(60);
        let mut body = serde_json::json!({
            "statement": sql,
            "timeout": timeout_secs
        });

        if let Some(ref db) = self.config.database {
            body["database"] = Value::String(db.clone());
        }
        if let Some(ref schema) = self.config.schema {
            body["schema"] = Value::String(schema.clone());
        }
        if let Some(ref wh) = self.config.warehouse {
            body["warehouse"] = Value::String(wh.clone());
        }
        if let Some(ref role) = self.config.role {
            body["role"] = Value::String(role.clone());
        }
        // Attach the query tag as a session parameter, so every statement is
        // attributable in Snowflake's QUERY_HISTORY.QUERY_TAG. The SQL API's
        // `parameters` object carries session parameters for this statement.
        if let Some(ref tag) = self.config.query_tag {
            body["parameters"] = serde_json::json!({ "QUERY_TAG": tag });
        }

        // A 401 means the token expired (Nimbus tokens last ~10 min) — Snowflake
        // rejects at auth, before running the statement, so retrying after a fresh
        // token is safe. Re-authenticate once, then give up.
        let mut reauthed = false;
        loop {
            let response = self
                .client
                .post(&url)
                .header("Content-Type", "application/json")
                .header("Accept", "application/json")
                .header("Authorization", format!("Bearer {}", self.current_token()))
                .header("X-Snowflake-Authorization-Token-Type", "OAUTH")
                .json(&body)
                .send()
                .map_err(|e| DatabaseError::QueryError(format!("HTTP request failed: {e}")))?;

            let status = response.status();
            let resp_body: Value = response
                .json()
                .map_err(|e| DatabaseError::QueryError(format!("Failed to parse response: {e}")))?;

            return match status.as_u16() {
                200 => Ok(resp_body),
                202 => {
                    // Async execution — poll for result
                    let handle = resp_body["statementHandle"]
                        .as_str()
                        .ok_or_else(|| {
                            DatabaseError::QueryError("No statementHandle in 202 response".into())
                        })?
                        .to_string();
                    self.poll_statement(&handle)
                }
                401 if !reauthed => {
                    tracing::warn!("Snowflake returned 401 — re-authenticating and retrying once");
                    self.refresh_token()?;
                    reauthed = true;
                    continue;
                }
                422 => {
                    let msg = resp_body["message"]
                        .as_str()
                        .unwrap_or("SQL compilation error");
                    Err(DatabaseError::QueryError(format!(
                        "Snowflake SQL error: {msg}"
                    )))
                }
                _ => {
                    let msg = resp_body["message"].as_str().unwrap_or("Unknown error");
                    Err(DatabaseError::QueryError(format!(
                        "Snowflake API error ({status}): {msg}"
                    )))
                }
            };
        }
    }

    /// Poll for a completed async statement.
    fn poll_statement(&self, handle: &str) -> Result<Value, DatabaseError> {
        let url = format!("{}/api/v2/statements/{}", self.config.base_url, handle);

        let mut backoff_ms = INITIAL_BACKOFF_MS;

        for _ in 0..MAX_POLL_ATTEMPTS {
            std::thread::sleep(std::time::Duration::from_millis(backoff_ms));

            let response = self
                .client
                .get(&url)
                .header("Authorization", format!("Bearer {}", self.current_token()))
                .header("X-Snowflake-Authorization-Token-Type", "OAUTH")
                .send()
                .map_err(|e| DatabaseError::QueryError(format!("Poll request failed: {e}")))?;

            let status = response.status();
            let body: Value = response.json().map_err(|e| {
                DatabaseError::QueryError(format!("Failed to parse poll response: {e}"))
            })?;

            match status.as_u16() {
                200 => return Ok(body),
                202 => {
                    // Still running — increase backoff (capped at 5s)
                    backoff_ms = (backoff_ms * 2).min(5000);
                    continue;
                }
                _ => {
                    let msg = body["message"].as_str().unwrap_or("Unknown error");
                    return Err(DatabaseError::QueryError(format!(
                        "Snowflake poll error ({status}): {msg}"
                    )));
                }
            }
        }

        Err(DatabaseError::QueryError(format!(
            "Snowflake statement {handle} timed out after {MAX_POLL_ATTEMPTS} poll attempts"
        )))
    }

    /// Fetch all partitions of a result set and combine them.
    ///
    /// Partition 0 is already in the initial response. Additional partitions
    /// are fetched via `GET /api/v2/statements/{handle}?partition=N`.
    fn fetch_all_partitions(&self, initial_response: &Value) -> Result<Vec<Value>, DatabaseError> {
        let mut all_data: Vec<Value> = Vec::new();

        // Partition 0 data is in the initial response
        if let Some(data) = initial_response["data"].as_array() {
            all_data.extend(data.iter().cloned());
        }

        // Check for additional partitions
        let num_partitions = initial_response["resultSetMetaData"]["partitionInfo"]
            .as_array()
            .map(|a| a.len())
            .unwrap_or(1);

        if num_partitions <= 1 {
            return Ok(all_data);
        }

        let handle = initial_response["statementHandle"]
            .as_str()
            .ok_or_else(|| DatabaseError::QueryError("No statementHandle for pagination".into()))?;

        for partition in 1..num_partitions {
            let url = format!(
                "{}/api/v2/statements/{}?partition={}",
                self.config.base_url, handle, partition
            );

            // Re-auth once on 401 (token may expire mid-pagination), and check the
            // status before decoding so a non-2xx surfaces its message instead of a
            // confusing "error decoding response body".
            let mut reauthed = false;
            let body: Value = loop {
                let response = self
                    .client
                    .get(&url)
                    .header("Accept", "application/json")
                    .header("Authorization", format!("Bearer {}", self.current_token()))
                    .header("X-Snowflake-Authorization-Token-Type", "OAUTH")
                    .send()
                    .map_err(|e| {
                        DatabaseError::QueryError(format!(
                            "Failed to fetch partition {partition}: {e}"
                        ))
                    })?;

                let status = response.status();
                if status.as_u16() == 401 && !reauthed {
                    self.refresh_token()?;
                    reauthed = true;
                    continue;
                }
                if !status.is_success() {
                    let msg = response.text().unwrap_or_default();
                    return Err(DatabaseError::QueryError(format!(
                        "Snowflake partition {partition} error ({status}): {msg}"
                    )));
                }
                break response.json().map_err(|e| {
                    DatabaseError::QueryError(format!(
                        "Failed to parse partition {partition} response: {e}"
                    ))
                })?;
            };

            if let Some(data) = body["data"].as_array() {
                all_data.extend(data.iter().cloned());
            }
        }

        Ok(all_data)
    }

    /// Parse a single cell value from the REST API response.
    ///
    /// Snowflake REST API returns all values as JSON strings (or null).
    /// We use `rowType` metadata to determine the target type.
    fn parse_cell(raw: &Value, sf_type: &str) -> Value {
        match raw {
            Value::Null => Value::Null,
            Value::String(s) => Self::parse_typed_string(s, sf_type),
            // The API shouldn't return non-string/non-null, but handle gracefully
            other => other.clone(),
        }
    }

    /// Parse a string value according to its Snowflake type.
    fn parse_typed_string(s: &str, sf_type: &str) -> Value {
        let sf_upper = sf_type.to_uppercase();

        match sf_upper.as_str() {
            // Fixed-point (integers and decimals)
            "FIXED" | "NUMBER" | "DECIMAL" | "NUMERIC" | "INT" | "INTEGER" | "BIGINT"
            | "SMALLINT" | "TINYINT" | "BYTEINT" => parse_text_value(s),

            // Floating point
            "FLOAT" | "FLOAT4" | "FLOAT8" | "DOUBLE" | "DOUBLE PRECISION" | "REAL" => s
                .parse::<f64>()
                .ok()
                .and_then(serde_json::Number::from_f64)
                .map(Value::Number)
                .unwrap_or_else(|| Value::String(s.to_string())),

            // Boolean
            "BOOLEAN" => match s {
                "true" | "TRUE" | "True" | "1" => Value::Bool(true),
                "false" | "FALSE" | "False" | "0" => Value::Bool(false),
                _ => Value::String(s.to_string()),
            },

            // Text/binary — keep as string
            "TEXT" | "VARCHAR" | "STRING" | "CHAR" | "CHARACTER" | "BINARY" | "VARBINARY" => {
                Value::String(s.to_string())
            }

            // Date/time — keep as string
            "DATE" | "TIME" | "TIMESTAMP" | "TIMESTAMP_LTZ" | "TIMESTAMP_NTZ" | "TIMESTAMP_TZ" => {
                Value::String(s.to_string())
            }

            // Semi-structured (VARIANT, OBJECT, ARRAY) — try to parse as JSON
            "VARIANT" | "OBJECT" | "ARRAY" => {
                serde_json::from_str(s).unwrap_or_else(|_| Value::String(s.to_string()))
            }

            // Fallback
            _ => parse_text_value(s),
        }
    }

    /// Convert the REST API response into Vec<Row>.
    fn response_to_rows(&self, response: &Value) -> Result<Vec<Row>, DatabaseError> {
        // Extract column metadata
        let row_type = response["resultSetMetaData"]["rowType"]
            .as_array()
            .ok_or_else(|| DatabaseError::QueryError("No rowType in response metadata".into()))?;

        let columns: Vec<(String, String)> = row_type
            .iter()
            .map(|col| {
                // Snowflake returns unquoted identifiers/aliases upper-cased
                // (WAREHOUSE_NAME), while the rest of the stack — SQL aliases,
                // the frontend types, and the Postgres/Redshift connector — use
                // lower snake_case. Normalise so callers get consistent keys.
                let name = col["name"].as_str().unwrap_or("?").to_lowercase();
                let type_name = col["type"].as_str().unwrap_or("TEXT").to_string();
                (name, type_name)
            })
            .collect();

        // Fetch all data (including additional partitions)
        let all_data = self.fetch_all_partitions(response)?;

        // Convert to rows
        let mut rows = Vec::with_capacity(all_data.len());
        for data_row in &all_data {
            let cells = data_row
                .as_array()
                .ok_or_else(|| DatabaseError::QueryError("Expected array in data row".into()))?;

            let mut row = Row::default();
            for (i, (name, type_name)) in columns.iter().enumerate() {
                let raw = cells.get(i).unwrap_or(&Value::Null);
                let value = Self::parse_cell(raw, type_name);
                row.insert(name.clone(), value);
            }
            rows.push(row);
        }

        Ok(rows)
    }
}

impl Database for SnowflakeDatabase {
    fn query(&self, sql: &str, params: &[Value]) -> QueryResult {
        let query_str = substitute_params(sql, params);

        let response = self.submit_statement(&query_str)?;

        // Check if this is a DDL/DML that doesn't return data
        let num_rows = response["resultSetMetaData"]["numRows"]
            .as_u64()
            .unwrap_or(0);

        if num_rows == 0 && response["data"].is_null() {
            return Ok(Vec::new());
        }

        self.response_to_rows(&response)
    }

    fn execute(&self, sql: &str, params: &[Value]) -> Result<u64, DatabaseError> {
        let query_str = substitute_params(sql, params);

        let response = self.submit_statement(&query_str)?;

        // For DML statements, numRowsInserted/Updated/Deleted are in statementStatusUrl
        // but numRows in metadata tells us affected rows for many cases.
        let num_rows = response["resultSetMetaData"]["numRows"]
            .as_u64()
            .unwrap_or(0);

        Ok(num_rows)
    }

    fn ping(&self) -> Result<(), DatabaseError> {
        self.query("SELECT 1", &[])?;
        Ok(())
    }

    fn close(&self) -> Result<(), DatabaseError> {
        Ok(())
    }

    fn get_table_schema(&self, table: &str) -> Result<crate::schema::TableSchema, DatabaseError> {
        let (schema_name, table_name) = parse_table_name(table);
        let schema_filter = schema_name
            .clone()
            .unwrap_or_else(|| self.default_schema.clone());

        let sql = format!(
            "SELECT column_name, data_type, ordinal_position, is_nullable \
             FROM information_schema.columns \
             WHERE table_schema = '{}' AND table_name = '{}' \
             ORDER BY ordinal_position",
            schema_filter.replace('\'', "''"),
            table_name.replace('\'', "''")
        );

        let rows = self.query(&sql, &[])?;
        schema_from_rows(rows, schema_name, table_name, &self.default_schema)
    }
}

#[cfg(test)]
// 3.14 / etc. sont des valeurs de test, pas des approximations de PI.
#[allow(clippy::approx_constant)]
mod tests {
    use super::*;

    #[test]
    fn test_snowflake_config_from_env() {
        unsafe {
            std::env::set_var("SNOWFLAKE_ACCOUNT", "myaccount");
            std::env::set_var("SNOWFLAKE_OAUTH_TOKEN", "mytoken");
            std::env::set_var("SNOWFLAKE_WAREHOUSE", "WH");
            std::env::set_var("SNOWFLAKE_DATABASE", "DB");
            std::env::set_var("SNOWFLAKE_SCHEMA", "PUBLIC");
        }

        let config = SnowflakeConfig::from_env();

        assert_eq!(config.account, "myaccount");
        assert_eq!(config.oauth_token, "mytoken");
        assert_eq!(config.warehouse, Some("WH".to_string()));
        assert_eq!(config.database, Some("DB".to_string()));
        assert_eq!(config.schema, Some("PUBLIC".to_string()));
    }

    #[test]
    fn auth_mode_parses_known_values_case_insensitively() {
        assert_eq!(AuthMode::parse("oauth").unwrap(), AuthMode::OAuth);
        assert_eq!(AuthMode::parse("  ENV ").unwrap(), AuthMode::Env);
    }

    #[test]
    fn auth_mode_rejects_unknown_value() {
        assert!(AuthMode::parse("carrier-pigeon").is_err());
    }

    #[test]
    fn query_tag_follows_nimbus_convention() {
        let tag = compose_query_tag("snowhouse", "production");
        let v: Value = serde_json::from_str(&tag).unwrap();
        assert_eq!(v["nimbus@source"], "snowhouse");
        assert_eq!(v["nimbus@owner"], "system");
        assert_eq!(v["nimbus@env"], "production");
    }

    #[test]
    fn credits_per_hour_doubles_each_size_up() {
        assert_eq!(credits_per_hour("X-Small"), Some(1.0));
        assert_eq!(credits_per_hour("Small"), Some(2.0));
        assert_eq!(credits_per_hour("Medium"), Some(4.0));
        assert_eq!(credits_per_hour("Large"), Some(8.0));
        assert_eq!(credits_per_hour("X-Large"), Some(16.0));
        assert_eq!(credits_per_hour("4X-Large"), Some(128.0));
    }

    #[test]
    fn credits_per_hour_is_case_and_form_insensitive() {
        // Snowflake reports sizes in mixed case and, historically, both the
        // "X-Small" and "XSMALL" spellings.
        assert_eq!(credits_per_hour("  large  "), Some(8.0));
        assert_eq!(credits_per_hour("XLARGE"), Some(16.0));
    }

    #[test]
    fn credits_per_hour_unknown_size_is_none() {
        assert_eq!(credits_per_hour("7X-Large"), None);
        assert_eq!(credits_per_hour(""), None);
    }

    #[test]
    fn large_is_the_warning_threshold() {
        // Large and up are worth a warning; Medium and below stay informational.
        assert!(credits_per_hour("Medium").unwrap() < LARGE_WAREHOUSE_CREDITS);
        assert!(credits_per_hour("Large").unwrap() >= LARGE_WAREHOUSE_CREDITS);
    }

    #[test]
    fn cheapest_picks_the_smallest_size() {
        let picked = cheapest_warehouse([("BIG", "Large"), ("MID", "Medium"), ("TINY", "X-Small")]);
        assert_eq!(picked.as_deref(), Some("TINY"));
    }

    #[test]
    fn cheapest_breaks_ties_on_name() {
        // Two X-Small warehouses — the lexicographically smaller name wins, so
        // the pick is stable across restarts rather than SHOW-order-dependent.
        let picked = cheapest_warehouse([("WH_B", "X-Small"), ("WH_A", "X-Small")]);
        assert_eq!(picked.as_deref(), Some("WH_A"));
    }

    #[test]
    fn cheapest_ranks_unknown_size_last() {
        // An unrecognised size must never be chosen over a known small one — it
        // could be larger than anything we can rank.
        let picked = cheapest_warehouse([("WEIRD", "Nano"), ("NORMAL", "Small")]);
        assert_eq!(picked.as_deref(), Some("NORMAL"));
    }

    #[test]
    fn cheapest_of_nothing_is_none() {
        assert_eq!(cheapest_warehouse([]), None);
    }

    #[test]
    fn response_to_rows_lowercases_snowflake_column_names() {
        let db = SnowflakeDatabase::connect_with_config(SnowflakeConfig {
            account: "acct".to_string(),
            base_url: default_base_url("acct"),
            oauth_token: "tok".to_string(),
            warehouse: None,
            database: None,
            schema: None,
            role: None,
            statement_timeout: None,
            query_tag: None,
        })
        .unwrap();

        // Snowflake upper-cases unquoted aliases in `rowType[].name`.
        let response = serde_json::json!({
            "resultSetMetaData": {
                "rowType": [
                    {"name": "WAREHOUSE_NAME", "type": "TEXT"},
                    {"name": "GB_SCANNED", "type": "FIXED"}
                ]
            },
            "data": [["WH1", "42"]]
        });

        let rows = db.response_to_rows(&response).unwrap();
        assert_eq!(rows.len(), 1);
        // Keys are lower snake_case, matching the frontend + Postgres/Redshift.
        assert!(rows[0].contains_key("warehouse_name"));
        assert!(rows[0].contains_key("gb_scanned"));
        assert!(!rows[0].contains_key("WAREHOUSE_NAME"));
    }

    #[test]
    fn test_api_url() {
        let config = SnowflakeConfig {
            account: "xy12345.eu-west-1".to_string(),
            base_url: default_base_url("xy12345.eu-west-1"),
            oauth_token: "token".to_string(),
            warehouse: None,
            database: None,
            schema: None,
            role: None,
            statement_timeout: None,
            query_tag: None,
        };
        assert_eq!(
            config.api_url(),
            "https://xy12345.eu-west-1.snowflakecomputing.com/api/v2/statements"
        );
    }

    #[test]
    fn test_parse_typed_string_fixed() {
        assert_eq!(
            SnowflakeDatabase::parse_typed_string("42", "FIXED"),
            Value::Number(42.into())
        );
        assert_eq!(
            SnowflakeDatabase::parse_typed_string("3.14", "NUMBER"),
            Value::Number(serde_json::Number::from_f64(3.14).unwrap())
        );
    }

    #[test]
    fn test_parse_typed_string_boolean() {
        assert_eq!(
            SnowflakeDatabase::parse_typed_string("true", "BOOLEAN"),
            Value::Bool(true)
        );
        assert_eq!(
            SnowflakeDatabase::parse_typed_string("1", "BOOLEAN"),
            Value::Bool(true)
        );
        assert_eq!(
            SnowflakeDatabase::parse_typed_string("false", "BOOLEAN"),
            Value::Bool(false)
        );
    }

    #[test]
    fn test_parse_typed_string_text() {
        assert_eq!(
            SnowflakeDatabase::parse_typed_string("hello", "TEXT"),
            Value::String("hello".to_string())
        );
        // Numeric string kept as string for TEXT type
        assert_eq!(
            SnowflakeDatabase::parse_typed_string("42", "VARCHAR"),
            Value::String("42".to_string())
        );
    }

    #[test]
    fn test_parse_typed_string_variant_json() {
        let result = SnowflakeDatabase::parse_typed_string(r#"{"key": "val"}"#, "VARIANT");
        assert!(result.is_object());
        assert_eq!(result["key"], Value::String("val".to_string()));
    }

    #[test]
    fn test_parse_cell_null() {
        assert_eq!(
            SnowflakeDatabase::parse_cell(&Value::Null, "FIXED"),
            Value::Null
        );
    }

    #[test]
    fn test_snowflake_requires_account_and_token() {
        let config = SnowflakeConfig {
            account: String::new(),
            base_url: default_base_url(""),
            oauth_token: "token".to_string(),
            warehouse: None,
            database: None,
            schema: None,
            role: None,
            statement_timeout: None,
            query_tag: None,
        };
        assert!(SnowflakeDatabase::connect_with_config(config).is_err());
    }
}
