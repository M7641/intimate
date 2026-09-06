//! Spike runner for the async Postgres/Redshift connector.
//!
//! Run against any reachable Postgres. Easiest: the local dev container from
//! `data_view start` (or a test container), pointing the `REDSHIFT_*` vars at it:
//!
//! ```sh
//! REDSHIFT_HOST=127.0.0.1 REDSHIFT_PORT=5432 \
//! REDSHIFT_DATABASE=postgres REDSHIFT_USERNAME=postgres \
//! REDSHIFT_PASSWORD=postgres REDSHIFT_SSL_MODE=disable \
//! cargo run -p database --features postgres-async --example postgres_async
//! ```
//!
//! Against a real Redshift cluster, drop `REDSHIFT_SSL_MODE` (defaults to
//! `require`) and set host/credentials accordingly.

use database::connectors::external::postgres_async::AsyncPostgresDatabase;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let db = AsyncPostgresDatabase::connect()?;

    db.ping().await?;
    println!("ping: ok");

    let rows = db
        .query(
            "SELECT 1 AS one, 'hello'::text AS greeting, 3.14::float8 AS pi",
            &[],
        )
        .await?;
    println!("{}", serde_json::to_string_pretty(&rows)?);

    Ok(())
}
