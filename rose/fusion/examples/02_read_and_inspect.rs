//! Example 02: Read Parquet back into Arrow, inspect schema and data
//!
//! Run: cargo run --example 02_read_and_inspect
//!
//! Prerequisite: run example 01 first to create data/sensors.parquet

use std::fs::File;

use arrow::util::pretty::print_batches;
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
use parquet::arrow::ProjectionMask;

fn main() {
    let path = "data/sensors.parquet";
    let file = File::open(path).expect("Run example 01 first to create data/sensors.parquet");

    // ── Full read ──────────────────────────────────────────────
    let builder = ParquetRecordBatchReaderBuilder::try_new(file).unwrap();

    // Inspect Parquet metadata before reading any data
    let parquet_meta = builder.metadata();
    let file_meta = parquet_meta.file_metadata();
    println!("=== Parquet File Metadata ===");
    println!("  version:    {}", file_meta.version());
    println!("  created by: {:?}", file_meta.created_by());
    println!("  num rows:   {}", file_meta.num_rows());
    println!("  row groups: {}", parquet_meta.num_row_groups());
    println!();

    // Inspect the Arrow schema derived from the Parquet schema
    let schema = builder.schema();
    println!("=== Arrow Schema ===");
    for (i, field) in schema.fields().iter().enumerate() {
        println!(
            "  [{i}] {:16} {:12} nullable={}",
            field.name(),
            field.data_type(),
            field.is_nullable()
        );
    }
    println!();

    // Read all data with a configurable batch size
    let reader = builder.with_batch_size(8).build().unwrap();

    let mut all_batches = Vec::new();
    for batch_result in reader {
        let batch = batch_result.unwrap();
        println!("Read batch: {} rows", batch.num_rows());
        all_batches.push(batch);
    }
    println!("\n=== All Data ===");
    print_batches(&all_batches).unwrap();

    // ── Projection: read only specific columns ────────────────
    println!("\n=== Projected Read (sensor_id, temperature only) ===");

    let file = File::open(path).unwrap();
    let builder = ParquetRecordBatchReaderBuilder::try_new(file).unwrap();
    let file_meta = builder.metadata().file_metadata().clone();

    // columns 0 (sensor_id) and 2 (temperature) only
    let mask = ProjectionMask::roots(file_meta.schema_descr(), [0, 2]);
    let reader = builder.with_projection(mask).build().unwrap();

    let projected: Vec<_> = reader.map(|b| b.unwrap()).collect();
    print_batches(&projected).unwrap();

    println!("\nDone! Run the next example: cargo run --example 03_compute_kernels");
}
