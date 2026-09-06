//! Example 04: Query Parquet data with SQL via DataFusion
//!
//! Run: cargo run --example 04_datafusion_sql
//!
//! Prerequisite: run example 01 first to create data/sensors.parquet

use datafusion::error::Result;
use datafusion::prelude::*;

#[tokio::main]
async fn main() -> Result<()> {
    // SessionContext is the entry point — it holds registered tables, config, and state
    let ctx = SessionContext::new();

    // Register our Parquet file as a table called "sensors"
    ctx.register_parquet(
        "sensors",
        "data/sensors.parquet",
        ParquetReadOptions::default(),
    )
    .await?;

    // ── Simple SELECT ────────────────────────────────────────
    println!("=== All sensor data ===");
    ctx.sql("SELECT * FROM sensors")
        .await?
        .show() // .show() collects and pretty-prints
        .await?;

    // ── Filtering with WHERE ─────────────────────────────────
    println!("=== Hot readings (temperature > 22.0) ===");
    ctx.sql(
        "SELECT sensor_id, location, temperature
             FROM sensors
             WHERE temperature > 22.0
             ORDER BY temperature DESC",
    )
    .await?
    .show()
    .await?;

    // ── Aggregation with GROUP BY ────────────────────────────
    println!("=== Stats per location ===");
    ctx.sql(
        "SELECT
                location,
                COUNT(*) as readings,
                ROUND(AVG(temperature), 2) as avg_temp,
                ROUND(MIN(temperature), 2) as min_temp,
                ROUND(MAX(temperature), 2) as max_temp,
                ROUND(AVG(humidity), 2) as avg_humidity
             FROM sensors
             GROUP BY location
             ORDER BY location",
    )
    .await?
    .show()
    .await?;

    // ── Stats per sensor ─────────────────────────────────────
    println!("=== Stats per sensor ===");
    ctx.sql(
        "SELECT
                sensor_id,
                location,
                COUNT(*) as readings,
                ROUND(AVG(temperature), 2) as avg_temp,
                ROUND(AVG(humidity), 2) as avg_humidity,
                ROUND(AVG(pressure), 2) as avg_pressure
             FROM sensors
             GROUP BY sensor_id, location
             ORDER BY sensor_id",
    )
    .await?
    .show()
    .await?;

    // ── Examine the query plan ───────────────────────────────
    println!("=== Query Plan (EXPLAIN) ===");
    println!("This shows how DataFusion optimises the query:");
    println!("  - predicate pushdown into the Parquet reader");
    println!("  - projection pruning (only needed columns)");
    println!();
    ctx.sql(
        "EXPLAIN SELECT sensor_id, AVG(temperature)
             FROM sensors
             WHERE humidity > 50.0
             GROUP BY sensor_id",
    )
    .await?
    .show()
    .await?;

    // ── DataFrame API (alternative to SQL) ───────────────────
    println!("=== DataFrame API (no SQL) ===");
    let df = ctx.table("sensors").await?;
    df.filter(col("location").eq(lit("warehouse_b")))?
        .select(vec![col("sensor_id"), col("temperature"), col("humidity")])?
        .sort(vec![col("temperature").sort(true, true)])?
        .show()
        .await?;

    println!("Done! Run the next example: cargo run --example 05_arrow_ipc");
    Ok(())
}
