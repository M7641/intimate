//! Integration tests: the real data_view HTTP API against a disposable,
//! seeded Postgres container — exercised through both wire-compatible backends
//! (`postgres` and `amazon_redshift`, which share the Postgres driver).
//!
//! Run locally against Podman:
//!   DOCKER_HOST="unix://$(podman machine inspect --format \
//!     '{{.ConnectionInfo.PodmanSocket.Path}}')" \
//!   TESTCONTAINERS_RYUK_DISABLED=true cargo test -p data_view --test api_it

mod support;

use support::snowflake_mock::start_snowflake_mock;
use support::{free_port, get_json, require_container_engine, seed, start_server};
use testcontainers::runners::SyncRunner;
use testcontainers_modules::postgres::Postgres as PgImage;

/// Env wiring the Postgres/Redshift backend at the seeded container (no TLS).
fn wire_env(warehouse: &str, pg_port: u16) -> Vec<(&'static str, String)> {
    vec![
        ("DATA_WAREHOUSE_TYPE", warehouse.to_string()),
        ("REDSHIFT_HOST", "127.0.0.1".to_string()),
        ("REDSHIFT_PORT", pg_port.to_string()),
        ("REDSHIFT_DATABASE", "postgres".to_string()),
        ("REDSHIFT_USERNAME", "postgres".to_string()),
        ("REDSHIFT_PASSWORD", "postgres".to_string()),
        ("REDSHIFT_SSL_MODE", "disable".to_string()),
    ]
}

fn len(v: &serde_json::Value) -> usize {
    v.as_array()
        .map(|a| a.len())
        .unwrap_or_else(|| panic!("expected JSON array, got {v}"))
}

/// Drive every data-explorer endpoint and assert on its shape/content.
fn assert_core_endpoints(base: &str, warehouse: &str) {
    let ctx = |msg: &str| format!("[{warehouse}] {msg}");

    // schemas + tables
    let schemas = get_json(base, "/api/data_view/schemas").to_string();
    assert!(
        schemas.contains("stage"),
        "{}",
        ctx("schemas missing 'stage'")
    );
    assert!(
        schemas.contains("marts"),
        "{}",
        ctx("schemas missing 'marts'")
    );

    let tables = get_json(base, "/api/data_view/tables?schema=stage").to_string();
    assert!(
        tables.contains("customer_snapshot"),
        "{}",
        ctx("tables missing customer_snapshot")
    );
    assert!(
        tables.contains("order_snapshot"),
        "{}",
        ctx("tables missing order_snapshot")
    );

    // columns: business columns present, internal columns hidden
    let cols = get_json(
        base,
        "/api/data_view/columns/customer_snapshot?schema=stage",
    )
    .to_string();
    assert!(
        cols.contains("customer_name"),
        "{}",
        ctx("columns missing customer_name")
    );
    assert!(cols.contains("region"), "{}", ctx("columns missing region"));
    assert!(
        !cols.contains("row_hash"),
        "{}",
        ctx("columns leaked internal row_hash")
    );
    assert!(
        !cols.contains("batch_group_id"),
        "{}",
        ctx("columns leaked batch_group_id")
    );

    // timestamps: two snapshots for a versioned table, none for an unversioned one
    let ts = get_json(
        base,
        "/api/data_view/timestamps/customer_snapshot?schema=stage",
    );
    assert_eq!(len(&ts), 2, "{}", ctx("expected 2 snapshots"));
    assert!(
        ts.to_string().contains("2026-06-08"),
        "{}",
        ctx("missing latest snapshot")
    );
    let ts_unversioned = get_json(base, "/api/data_view/timestamps/region_lookup?schema=marts");
    assert_eq!(
        len(&ts_unversioned),
        0,
        "{}",
        ctx("unversioned table should have no snapshots")
    );

    // row counts: one entry per snapshot
    let rc = get_json(
        base,
        "/api/data_view/row_counts/customer_snapshot?schema=stage",
    );
    assert_eq!(len(&rc), 2, "{}", ctx("expected 2 row-count entries"));

    // data: default serves the latest snapshot (8 rows), internal columns hidden
    let latest = get_json(
        base,
        "/api/data_view/data/customer_snapshot?schema=stage&limit=1000",
    );
    assert_eq!(len(&latest), 8, "{}", ctx("latest snapshot row count"));
    let latest_s = latest.to_string();
    assert!(
        latest_s.contains("1200"),
        "{}",
        ctx("latest should show updated value 1200")
    );
    assert!(
        !latest_s.contains("row_hash"),
        "{}",
        ctx("data leaked internal row_hash")
    );

    // The frontend formats by JS type (`typeof === "number"`), so the API must
    // emit JSON numbers and bools, not strings. This is the coercion the
    // Snowflake REST path (every value arrives as a string) is most likely to
    // get wrong; the Postgres driver returns native types and is the control.
    let first = &latest.as_array().expect("data is an array")[0];
    assert!(
        first["lifetime_value"].is_number(),
        "{}",
        ctx(&format!(
            "lifetime_value should be a JSON number, got {}",
            first["lifetime_value"]
        ))
    );
    assert!(
        first["is_active"].is_boolean(),
        "{}",
        ctx(&format!(
            "is_active should be a JSON bool, got {}",
            first["is_active"]
        ))
    );

    // data: an explicit older snapshot filters to that load
    let load1 = get_json(
        base,
        "/api/data_view/data/customer_snapshot?schema=stage&load_timestamp=2026-06-01%2000:00:00",
    );
    assert!(
        load1.to_string().contains("1000.5"),
        "{}",
        ctx("older snapshot value missing")
    );

    // column values
    let vals = get_json(
        base,
        "/api/data_view/column_values/customer_snapshot/region?schema=stage",
    )
    .to_string();
    for region in ["emea", "amer", "apac"] {
        assert!(
            vals.contains(region),
            "{}",
            ctx(&format!("column_values missing {region}"))
        );
    }

    // column stats on a numeric column — exercises MEDIAN/PERCENTILE_CONT, the
    // dialect-sensitive aggregates the Redshift-compat patch backfills.
    let stats = get_json(
        base,
        "/api/data_view/column_stats/customer_snapshot/lifetime_value?schema=stage",
    );
    assert!(
        stats.is_object(),
        "{}",
        ctx("column_stats should be an object")
    );
    assert!(
        stats.to_string().contains("count"),
        "{}",
        ctx("column_stats missing counts")
    );

    // value distribution — numeric (arithmetic binning) and categorical
    get_json(
        base,
        "/api/data_view/value_distribution/customer_snapshot/lifetime_value?schema=stage",
    );
    get_json(
        base,
        "/api/data_view/value_distribution/customer_snapshot/region?schema=stage",
    );

    // unversioned table: data with no timestamp concept
    let regions = get_json(base, "/api/data_view/data/region_lookup?schema=marts");
    assert_eq!(len(&regions), 3, "{}", ctx("region_lookup row count"));
    assert!(
        regions.to_string().contains("Japan"),
        "{}",
        ctx("region_lookup data missing")
    );
}

#[test]
fn core_endpoints_across_wire_backends() {
    require_container_engine();

    let node = PgImage::default()
        .start()
        .expect("start postgres container");
    let pg_port = node.get_host_port_ipv4(5432).expect("mapped postgres port");
    let dsn =
        format!("host=127.0.0.1 port={pg_port} user=postgres password=postgres dbname=postgres");
    seed(&dsn);

    // Both backends share the Postgres driver; assert the API behaves on each.
    for warehouse in ["postgres", "amazon_redshift"] {
        let port = free_port();
        let server = start_server(port, &wire_env(warehouse, pg_port));
        assert_core_endpoints(&server.base_url, warehouse);
        // `server` drops here -> process killed; `node` drops at fn end.
    }
}

#[test]
fn core_endpoints_via_snowflake_mock() {
    require_container_engine();

    let node = PgImage::default()
        .start()
        .expect("start postgres container");
    let pg_port = node.get_host_port_ipv4(5432).expect("mapped postgres port");
    let dsn =
        format!("host=127.0.0.1 port={pg_port} user=postgres password=postgres dbname=postgres");
    seed(&dsn);

    // Put the mock Snowflake REST endpoint in front of the same container, then
    // drive the server's `snowflake` backend (HTTP + response parsing + type
    // coercion) against it. The account/token just have to be non-empty; the
    // base-URL override sends every call to the mock instead of Snowflake.
    let mock = start_snowflake_mock(dsn.clone());
    let port = free_port();
    let envs = vec![
        ("DATA_WAREHOUSE_TYPE", "snowflake".to_string()),
        ("SNOWFLAKE_ACCOUNT", "mock".to_string()),
        ("SNOWFLAKE_OAUTH_TOKEN", "mock-token".to_string()),
        ("SNOWFLAKE_BASE_URL", mock.base_url.clone()),
    ];
    let server = start_server(port, &envs);
    assert_core_endpoints(&server.base_url, "snowflake");
}
