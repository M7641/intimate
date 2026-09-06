//! Example 05: Arrow IPC — the zero-copy inter-process bridge
//!
//! Run: cargo run --example 05_arrow_ipc
//!
//! Demonstrates:
//!   - Writing Arrow data to IPC File format (random access)
//!   - Writing Arrow data to IPC Stream format (sequential)
//!   - Reading both formats back
//!   - Comparing file sizes: Parquet (compressed) vs IPC (raw Arrow)
//!
//! Prerequisite: run example 01 first to create data/sensors.parquet

use std::fs::{self, File};
use std::sync::Arc;

use arrow::array::{Float64Array, Int32Array, Int64Array, StringArray};
use arrow::datatypes::{DataType, Field, Schema};
use arrow::ipc::reader::{FileReader, StreamReader};
use arrow::ipc::writer::{FileWriter, StreamWriter};
use arrow::record_batch::RecordBatch;
use arrow::util::pretty::print_batches;

fn main() {
    let schema = Arc::new(Schema::new(vec![
        Field::new("sensor_id", DataType::Int32, false),
        Field::new("location", DataType::Utf8, false),
        Field::new("temperature", DataType::Float64, false),
        Field::new("humidity", DataType::Float64, false),
        Field::new("timestamp", DataType::Int64, false),
    ]));

    // Create two batches to demonstrate multi-batch IPC
    let batch1 = RecordBatch::try_new(
        schema.clone(),
        vec![
            Arc::new(Int32Array::from(vec![1, 2, 3])),
            Arc::new(StringArray::from(vec![
                "warehouse_a",
                "warehouse_a",
                "warehouse_b",
            ])),
            Arc::new(Float64Array::from(vec![22.1, 23.4, 19.8])),
            Arc::new(Float64Array::from(vec![45.0, 47.2, 55.1])),
            Arc::new(Int64Array::from(vec![1700000000, 1700000000, 1700000000])),
        ],
    )
    .unwrap();

    let batch2 = RecordBatch::try_new(
        schema.clone(),
        vec![
            Arc::new(Int32Array::from(vec![1, 2, 3])),
            Arc::new(StringArray::from(vec![
                "warehouse_a",
                "warehouse_a",
                "warehouse_b",
            ])),
            Arc::new(Float64Array::from(vec![22.5, 23.1, 20.1])),
            Arc::new(Float64Array::from(vec![44.8, 46.9, 54.8])),
            Arc::new(Int64Array::from(vec![1700000060, 1700000060, 1700000060])),
        ],
    )
    .unwrap();

    // ── IPC File Format (random access) ──────────────────────
    let ipc_file_path = "data/sensors.arrow";
    {
        let file = File::create(ipc_file_path).unwrap();
        let mut writer = FileWriter::try_new(file, &schema).unwrap();
        writer.write(&batch1).unwrap();
        writer.write(&batch2).unwrap();
        writer.finish().unwrap();
    }
    println!("=== IPC File Format ===");
    println!("Wrote {ipc_file_path}");

    // Read it back — notice we get random access to batches
    let file = File::open(ipc_file_path).unwrap();
    let reader = FileReader::try_new(file, None).unwrap();
    println!("  schema: {:?}", reader.schema());
    println!("  num batches: {}", reader.num_batches());
    let file_batches: Vec<_> = reader.map(|b| b.unwrap()).collect();
    print_batches(&file_batches).unwrap();
    println!();

    // ── IPC Stream Format (sequential, good for pipes/sockets) ──
    println!("=== IPC Stream Format ===");
    let mut buffer: Vec<u8> = Vec::new();
    {
        let mut writer = StreamWriter::try_new(&mut buffer, &schema).unwrap();
        writer.write(&batch1).unwrap();
        writer.write(&batch2).unwrap();
        writer.finish().unwrap();
    }
    println!("Wrote {} bytes to in-memory stream", buffer.len());

    // Read back from the stream
    let cursor = std::io::Cursor::new(&buffer);
    let reader = StreamReader::try_new(cursor, None).unwrap();
    let stream_batches: Vec<_> = reader.map(|b| b.unwrap()).collect();
    print_batches(&stream_batches).unwrap();
    println!();

    // ── Size comparison ──────────────────────────────────────
    println!("=== Format Size Comparison ===");
    let parquet_size = fs::metadata("data/sensors.parquet")
        .map(|m| m.len())
        .unwrap_or(0);
    let ipc_file_size = fs::metadata(ipc_file_path).map(|m| m.len()).unwrap_or(0);
    let ipc_stream_size = buffer.len() as u64;

    println!(
        "  Parquet (SNAPPY):  {:>6} bytes  ← compressed, optimised for storage",
        parquet_size
    );
    println!(
        "  IPC File:          {:>6} bytes  ← raw Arrow layout, random access",
        ipc_file_size
    );
    println!(
        "  IPC Stream:        {:>6} bytes  ← raw Arrow layout, sequential",
        ipc_stream_size
    );
    println!();
    println!("  Parquet is smaller because it compresses columnar data.");
    println!("  IPC is larger but requires ZERO decoding — the receiver");
    println!("  gets Arrow arrays ready to use. That's the trade-off:");
    println!("  storage efficiency (Parquet) vs transfer speed (IPC).");

    println!("\nDone! Run the next example: cargo run --example 06_datafusion_analytics");
}
