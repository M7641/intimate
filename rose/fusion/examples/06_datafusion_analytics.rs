//! Example 06: Advanced DataFusion analytics
//!
//! Run: cargo run --example 06_datafusion_analytics
//!
//! Demonstrates:
//!   - Window functions (running averages, ranking, lag)
//!   - CTEs (WITH clauses)
//!   - In-memory tables (MemTable) for reference data joins
//!   - Writing query results back to Parquet
//!
//! Prerequisite: run example 01 first to create data/sensors.parquet

use std::sync::Arc;

use arrow::array::{Float64Array, Int32Array, StringArray};
use arrow::datatypes::{DataType, Field, Schema};
use arrow::record_batch::RecordBatch;
use datafusion::datasource::MemTable;
use datafusion::error::Result;
use datafusion::prelude::*;

#[tokio::main]
async fn main() -> Result<()> {
    let ctx = SessionContext::new();

    ctx.register_parquet(
        "sensors",
        "data/sensors.parquet",
        ParquetReadOptions::default(),
    )
    .await?;

    // ── Window Functions: running average per sensor ─────────
    println!("=== Window Function: running avg temperature per sensor ===");
    ctx.sql(
        "
        SELECT
            sensor_id,
            timestamp,
            temperature,
            ROUND(AVG(temperature) OVER (
                PARTITION BY sensor_id
                ORDER BY timestamp
                ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW
            ), 2) as running_avg_temp
        FROM sensors
        ORDER BY sensor_id, timestamp
    ",
    )
    .await?
    .show()
    .await?;

    // ── Window: temperature delta from previous reading ──────
    println!("=== LAG: temperature change from previous reading ===");
    ctx.sql(
        "
        SELECT
            sensor_id,
            timestamp,
            temperature,
            LAG(temperature) OVER (
                PARTITION BY sensor_id ORDER BY timestamp
            ) as prev_temp,
            ROUND(temperature - COALESCE(
                LAG(temperature) OVER (PARTITION BY sensor_id ORDER BY timestamp),
                temperature
            ), 2) as temp_delta
        FROM sensors
        ORDER BY sensor_id, timestamp
    ",
    )
    .await?
    .show()
    .await?;

    // ── Window: rank sensors by average temperature ──────────
    println!("=== RANK: sensors ranked by avg temperature ===");
    ctx.sql(
        "
        WITH sensor_stats AS (
            SELECT
                sensor_id,
                location,
                ROUND(AVG(temperature), 2) as avg_temp,
                ROUND(AVG(humidity), 2) as avg_humidity,
                COUNT(*) as num_readings
            FROM sensors
            GROUP BY sensor_id, location
        )
        SELECT
            *,
            RANK() OVER (ORDER BY avg_temp DESC) as temp_rank
        FROM sensor_stats
        ORDER BY temp_rank
    ",
    )
    .await?
    .show()
    .await?;

    // ── MemTable: join with in-memory reference data ─────────
    println!("=== JOIN with in-memory lookup table ===");

    // Create a reference table: sensor thresholds
    let threshold_schema = Arc::new(Schema::new(vec![
        Field::new("sensor_id", DataType::Int32, false),
        Field::new("max_temp", DataType::Float64, false),
        Field::new("max_humidity", DataType::Float64, false),
        Field::new("owner", DataType::Utf8, false),
    ]));

    let threshold_batch = RecordBatch::try_new(
        threshold_schema.clone(),
        vec![
            Arc::new(Int32Array::from(vec![1, 2, 3])),
            Arc::new(Float64Array::from(vec![22.0, 23.5, 20.0])),
            Arc::new(Float64Array::from(vec![46.0, 47.0, 55.0])),
            Arc::new(StringArray::from(vec![
                "ops_team_a",
                "ops_team_a",
                "ops_team_b",
            ])),
        ],
    )?;

    // Register as a MemTable — Arrow data directly queryable via SQL
    let mem_table = MemTable::try_new(threshold_schema, vec![vec![threshold_batch]])?;
    ctx.register_table("thresholds", Arc::new(mem_table))?;

    // Find readings that exceed thresholds
    ctx.sql(
        "
        SELECT
            s.sensor_id,
            s.location,
            s.temperature,
            t.max_temp,
            s.humidity,
            t.max_humidity,
            t.owner,
            CASE
                WHEN s.temperature > t.max_temp THEN 'TEMP_ALERT'
                WHEN s.humidity > t.max_humidity THEN 'HUMIDITY_ALERT'
                ELSE 'OK'
            END as status
        FROM sensors s
        JOIN thresholds t ON s.sensor_id = t.sensor_id
        WHERE s.temperature > t.max_temp
           OR s.humidity > t.max_humidity
        ORDER BY s.sensor_id, s.timestamp
    ",
    )
    .await?
    .show()
    .await?;

    // ── Write query results back to Parquet ──────────────────
    println!("=== Write aggregated results back to Parquet ===");
    let result_df = ctx
        .sql(
            "
        SELECT
            sensor_id,
            location,
            ROUND(AVG(temperature), 2) as avg_temp,
            ROUND(AVG(humidity), 2) as avg_humidity,
            ROUND(AVG(pressure), 2) as avg_pressure,
            MIN(timestamp) as first_reading,
            MAX(timestamp) as last_reading,
            COUNT(*) as num_readings
        FROM sensors
        GROUP BY sensor_id, location
        ORDER BY sensor_id
    ",
        )
        .await?;

    result_df.clone().show().await?;

    // DataFusion can write results directly to Parquet
    result_df
        .write_parquet("data/sensor_summary.parquet", Default::default(), None)
        .await?;
    println!("Wrote data/sensor_summary.parquet");

    // Verify by reading it back
    println!("\n=== Verify: read summary back ===");
    ctx.register_parquet(
        "summary",
        "data/sensor_summary.parquet",
        ParquetReadOptions::default(),
    )
    .await?;
    ctx.sql("SELECT * FROM summary").await?.show().await?;

    println!("Done! You've explored the full Arrow → Parquet → DataFusion pipeline.");
    Ok(())
}
