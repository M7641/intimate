//! Example 03: Arrow compute kernels — filter, sort, aggregate
//!
//! Run: cargo run --example 03_compute_kernels
//!
//! Prerequisite: run example 01 first to create data/sensors.parquet

use arrow::array::{Array, AsArray, Float64Array, Int32Array, RecordBatch, StringArray};
use arrow::compute::kernels::aggregate::{max, min, sum};
use arrow::compute::kernels::boolean::and;
use arrow::compute::kernels::cmp::{eq, gt, lt};
use arrow::compute::kernels::filter::filter;
use arrow::compute::kernels::sort::{sort_to_indices, SortOptions};
use arrow::compute::kernels::take::take;
use arrow::datatypes::Float64Type;
use arrow::util::pretty::print_batches;
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
use std::fs::File;

fn main() {
    // Read back our sensor data
    let file = File::open("data/sensors.parquet").expect("Run example 01 first");
    let reader = ParquetRecordBatchReaderBuilder::try_new(file)
        .unwrap()
        .build()
        .unwrap();
    let batches: Vec<RecordBatch> = reader.map(|b| b.unwrap()).collect();
    let batch = &batches[0]; // we wrote a single batch

    let temperatures = batch.column(2).as_primitive::<Float64Type>();
    let humidities = batch.column(3).as_primitive::<Float64Type>();
    let sensor_ids = batch
        .column(0)
        .as_any()
        .downcast_ref::<Int32Array>()
        .unwrap();
    let locations = batch
        .column(1)
        .as_any()
        .downcast_ref::<StringArray>()
        .unwrap();

    // ── Aggregation ──────────────────────────────────────────
    println!("=== Aggregations ===");
    println!(
        "  temperature sum: {:.1}",
        sum::<Float64Type>(temperatures).unwrap()
    );
    println!(
        "  temperature min: {:.1}",
        min::<Float64Type>(temperatures).unwrap()
    );
    println!(
        "  temperature max: {:.1}",
        max::<Float64Type>(temperatures).unwrap()
    );
    println!(
        "  humidity sum:    {:.1}",
        sum::<Float64Type>(humidities).unwrap()
    );
    println!();

    // ── Filtering: temperature > 22.0 ────────────────────────
    println!("=== Filter: temperature > 22.0 ===");
    let threshold = Float64Array::new_scalar(22.0);
    let predicate = gt(temperatures, &threshold).unwrap();
    let filtered_temps = filter(temperatures, &predicate).unwrap();
    let filtered_ids = filter(sensor_ids, &predicate).unwrap();
    let filtered_locations = filter(locations, &predicate).unwrap();

    let filtered_batch = RecordBatch::try_from_iter(vec![
        ("sensor_id", filtered_ids),
        ("location", filtered_locations),
        ("temperature", filtered_temps),
    ])
    .unwrap();
    print_batches(&[filtered_batch]).unwrap();
    println!();

    // ── Compound filter: temperature > 20.0 AND temperature < 23.0 ──
    println!("=== Filter: 20.0 < temperature < 23.0 ===");
    let low = Float64Array::new_scalar(20.0);
    let high = Float64Array::new_scalar(23.0);
    let above_low = gt(temperatures, &low).unwrap();
    let below_high = lt(temperatures, &high).unwrap();
    let in_range = and(&above_low, &below_high).unwrap();
    let range_temps = filter(temperatures, &in_range).unwrap();
    println!("  {} readings in range", range_temps.len());
    println!();

    // ── String equality filter: location == "warehouse_b" ───
    println!("=== Filter: location == 'warehouse_b' ===");
    let target = StringArray::new_scalar("warehouse_b");
    let loc_match = eq(locations, &target).unwrap();
    let wb_temps = filter(temperatures, &loc_match).unwrap();
    let wb_humid = filter(humidities, &loc_match).unwrap();
    println!("  warehouse_b readings: {}", wb_temps.len());
    println!(
        "  avg temperature: {:.1}",
        sum::<Float64Type>(wb_temps.as_primitive::<Float64Type>()).unwrap() / wb_temps.len() as f64
    );
    println!(
        "  avg humidity:    {:.1}",
        sum::<Float64Type>(wb_humid.as_primitive::<Float64Type>()).unwrap() / wb_humid.len() as f64
    );
    println!();

    // ── Sorting by temperature (ascending) ──────────────────
    println!("=== Sort by temperature (ascending) ===");
    let indices = sort_to_indices(temperatures, Some(SortOptions::default()), None).unwrap();

    // Use the sort indices to reorder ALL columns together
    let sorted_temps = take(temperatures, &indices, None).unwrap();
    let sorted_ids = take(sensor_ids, &indices, None).unwrap();
    let sorted_locations = take(locations, &indices, None).unwrap();

    let sorted_batch = RecordBatch::try_from_iter(vec![
        ("sensor_id", sorted_ids),
        ("location", sorted_locations),
        ("temperature", sorted_temps),
    ])
    .unwrap();
    print_batches(&[sorted_batch]).unwrap();

    println!("\nDone! Run the next example: cargo run --example 04_datafusion_sql");
}
