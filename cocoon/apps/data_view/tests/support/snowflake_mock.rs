//! A mock of Snowflake's SQL REST API (`POST /api/v2/statements`), backed by the
//! same Postgres test container.
//!
//! There is no Snowflake test container, so this is how we exercise the real
//! Snowflake connector end to end: its request building, response parsing,
//! partition handling and — the bug-prone part — its string→typed coercion.
//! The connector points at us via `SNOWFLAKE_BASE_URL`.
//!
//! We translate by running the statement against Postgres and reshaping the
//! result into Snowflake's response JSON: a `rowType` (name + Snowflake type)
//! plus `data` rows where every cell is a string or null, exactly as the real
//! API returns them.

use std::sync::Arc;
use std::thread;

use serde_json::{Value, json};
use tiny_http::{Header, Response, Server};

/// A running mock server. Workers hold `Arc` clones; dropping this stops the
/// process from keeping the test alive (threads are detached daemons).
pub struct SnowflakeMock {
    pub base_url: String,
    _server: Arc<Server>,
}

/// Start the mock on an ephemeral port, proxying statements to `pg_dsn`.
pub fn start_snowflake_mock(pg_dsn: String) -> SnowflakeMock {
    let server = Arc::new(Server::http("127.0.0.1:0").expect("bind snowflake mock"));
    let port = server.server_addr().to_ip().expect("mock ip addr").port();
    let base_url = format!("http://127.0.0.1:{port}");

    // A few workers so the server's concurrent DB calls don't serialise behind
    // one another. Each request opens its own short-lived Postgres connection.
    for _ in 0..4 {
        let server = Arc::clone(&server);
        let dsn = pg_dsn.clone();
        thread::spawn(move || {
            while let Ok(request) = server.recv() {
                handle(request, &dsn);
            }
        });
    }

    SnowflakeMock {
        base_url,
        _server: server,
    }
}

fn handle(mut request: tiny_http::Request, dsn: &str) {
    let mut body = String::new();
    let _ = request.as_reader().read_to_string(&mut body);

    let statement = serde_json::from_str::<Value>(&body)
        .ok()
        .and_then(|v| v["statement"].as_str().map(str::to_owned))
        .unwrap_or_default();

    let payload = match run_statement(dsn, &statement) {
        Ok(v) => v,
        // Shape a 200 with Snowflake's error field; surfaces as a query error.
        Err(e) => json!({ "message": format!("mock execution failed: {e}") }),
    };

    let bytes = serde_json::to_vec(&payload).unwrap_or_default();
    let header = Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..]).unwrap();
    let _ = request.respond(Response::from_data(bytes).with_header(header));
}

/// Execute one statement and shape it into Snowflake's response JSON.
fn run_statement(dsn: &str, sql: &str) -> Result<Value, String> {
    let mut client = postgres::Client::connect(dsn, postgres::NoTls).map_err(|e| e.to_string())?;

    // Column names + types come from a prepared statement (no execution); the
    // text values come from a simple query (Postgres renders every value as
    // text). Both reflect the same SELECT, so they line up by index.
    let prepared = client.prepare(sql).map_err(|e| e.to_string())?;
    let sf_types: Vec<&'static str> = prepared
        .columns()
        .iter()
        .map(|c| snowflake_type(c.type_().name()))
        .collect();
    let row_type: Vec<Value> = prepared
        .columns()
        .iter()
        .map(|c| json!({ "name": c.name(), "type": snowflake_type(c.type_().name()) }))
        .collect();

    let mut data: Vec<Value> = Vec::new();
    for message in client.simple_query(sql).map_err(|e| e.to_string())? {
        if let postgres::SimpleQueryMessage::Row(row) = message {
            let cells: Vec<Value> = sf_types
                .iter()
                .enumerate()
                .map(|(i, ty)| match row.get(i) {
                    None => Value::Null,
                    Some(s) => Value::String(normalize(ty, s)),
                })
                .collect();
            data.push(Value::Array(cells));
        }
    }

    Ok(json!({
        "resultSetMetaData": {
            "numRows": data.len(),
            "rowType": row_type,
            "partitionInfo": [{}],
        },
        "data": data,
    }))
}

/// Map a Postgres type name to the Snowflake `rowType` type the connector keys
/// its coercion off. Only the broad class matters (number vs bool vs text).
fn snowflake_type(pg_type: &str) -> &'static str {
    match pg_type {
        "int2" | "int4" | "int8" | "numeric" => "FIXED",
        "float4" | "float8" => "REAL",
        "bool" => "BOOLEAN",
        _ => "TEXT",
    }
}

/// Postgres renders booleans as `t`/`f` in text; the connector expects
/// `true`/`false`. Everything else passes through untouched.
fn normalize(sf_type: &str, raw: &str) -> String {
    if sf_type == "BOOLEAN" {
        match raw {
            "t" => return "true".to_string(),
            "f" => return "false".to_string(),
            _ => {}
        }
    }
    raw.to_string()
}
