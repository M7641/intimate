//! Example 01: Create sensor data and write it to Parquet
//!
//! Run: cargo run --example 01_write_parquet
//!
//! This creates data/sensors.parquet which subsequent examples consume.

use std::fs::File;
use std::sync::Arc;

use arrow::array::{Float64Array, Int32Array, Int64Array, StringArray};
use arrow::datatypes::{DataType, Field, Schema};
use arrow::record_batch::RecordBatch;
use parquet::arrow::ArrowWriter;
use parquet::basic::Compression;
use parquet::file::properties::WriterProperties;

fn main() {
    // -- Schema: what columns exist and their types
    let schema = Arc::new(Schema::new(vec![
        Field::new("sensor_id", DataType::Int32, false),
        Field::new("location", DataType::Utf8, false),
        Field::new("temperature", DataType::Float64, false),
        Field::new("humidity", DataType::Float64, false),
        Field::new("pressure", DataType::Float64, true), // nullable — some sensors don't report pressure
        Field::new("timestamp", DataType::Int64, false), // unix epoch seconds
    ]));

    // -- Build columnar data as Arrow arrays
    //    Each array is one column. All arrays must have the same length.
    let sensor_ids = Int32Array::from(vec![1, 2, 3, 1, 2, 3, 1, 2, 3, 1, 2, 3]);
    let locations = StringArray::from(vec![
        "warehouse_a",
        "warehouse_a",
        "warehouse_b",
        "warehouse_a",
        "warehouse_a",
        "warehouse_b",
        "warehouse_a",
        "warehouse_a",
        "warehouse_b",
        "warehouse_a",
        "warehouse_a",
        "warehouse_b",
    ]);
    let temperatures = Float64Array::from(vec![
        22.1, 23.4, 19.8, 22.5, 23.1, 20.1, 21.9, 24.0, 19.5, 22.8, 23.7, 20.4,
    ]);
    let humidities = Float64Array::from(vec![
        45.0, 47.2, 55.1, 44.8, 46.9, 54.8, 45.5, 48.0, 55.6, 44.2, 47.5, 54.3,
    ]);
    // Pressure is nullable — sensor 3 doesn't report it
    let pressures = Float64Array::from(vec![
        Some(1013.2),
        Some(1013.5),
        None,
        Some(1013.1),
        Some(1013.4),
        None,
        Some(1013.3),
        Some(1013.6),
        None,
        Some(1013.0),
        Some(1013.3),
        None,
    ]);
    let timestamps = Int64Array::from(vec![
        1700000000, 1700000000, 1700000000, 1700000060, 1700000060, 1700000060, 1700000120,
        1700000120, 1700000120, 1700000180, 1700000180, 1700000180,
    ]);

    // -- Pack columns into a RecordBatch
    //    A RecordBatch is Arrow's fundamental unit: a schema + N equal-length arrays.
    let batch = RecordBatch::try_new(
        schema.clone(),
        vec![
            Arc::new(sensor_ids),
            Arc::new(locations),
            Arc::new(temperatures),
            Arc::new(humidities),
            Arc::new(pressures),
            Arc::new(timestamps),
        ],
    )
    .expect("Failed to create RecordBatch");

    println!(
        "Created RecordBatch: {} rows x {} columns",
        batch.num_rows(),
        batch.num_columns()
    );
    println!("Schema:\n{}", schema);

    // -- Write to Parquet with SNAPPY compression
    let props = WriterProperties::builder()
        .set_compression(Compression::SNAPPY)
        .set_max_row_group_size(1000)
        .build();

    let path = "data/sensors.parquet";
    let file = File::create(path).expect("Failed to create file");
    let mut writer =
        ArrowWriter::try_new(file, schema, Some(props)).expect("Failed to create ArrowWriter");

    writer.write(&batch).expect("Failed to write batch");
    let metadata = writer.close().expect("Failed to close writer");

    println!("\nWrote {path}");
    println!("  rows written: {}", metadata.num_rows);
    println!("  row groups:   {}", metadata.row_groups.len());
    for (i, rg) in metadata.row_groups.iter().enumerate() {
        println!(
            "  row group {i}: {} rows, {} bytes compressed",
            rg.num_rows,
            rg.total_compressed_size.unwrap_or(0),
        );
    }

    println!("\nDone! Run the next example: cargo run --example 02_read_and_inspect");
}
