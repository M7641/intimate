//! Hot-path benchmarks for the `parse` module, run via `tako bench-parse`.
//!
//! The hot path is [`parse_data`]: decode an in-memory upload (CSV / JSON /
//! Parquet / Avro / Vortex) into a Polars `DataFrame`. CSV/JSON/Parquet/Avro are
//! benched off a single synthetic `DataFrame` round-tripped through each writer;
//! Vortex is encoded separately (see `encode_vortex`). No external fixtures,
//! deterministic size, and `Throughput::Bytes` reports MB/s so formats compare.
//!
//! Criterion is driven through its programmatic API (not the `cargo bench`
//! harness) because the benchmark lives inside the binary rather than in a
//! separate `[[bench]]` target — see [`run`]. Results still land under
//! `target/criterion/`.

use std::hint::black_box;

use criterion::{BenchmarkId, Criterion, Throughput};
use polars::prelude::*;

use crate::parse::parse_data;

/// Entry point for the `bench-parse` subcommand: run every format's decode
/// benchmark, then print Criterion's summary.
pub fn run() {
    let mut c = Criterion::default();
    bench_parse_data(&mut c);
    bench_parse_vortex(&mut c);
    c.final_summary();
}

/// Row count for the synthetic dataset. Big enough that decode time dominates
/// setup noise, small enough to keep a bench round quick.
const ROWS: usize = 50_000;

/// Build a deterministic 3-column frame (int, low-cardinality string, float).
fn sample_frame() -> DataFrame {
    let ids: Vec<i64> = (0..ROWS as i64).collect();
    let categories: Vec<String> = (0..ROWS).map(|i| format!("cat_{:02}", i % 50)).collect();
    let values: Vec<f64> = (0..ROWS).map(|i| i as f64 * 1.5).collect();
    df!("id" => ids, "category" => categories, "value" => values)
        .expect("constructing the sample frame")
}

/// Encode the frame into a format's bytes, mirroring what `/upload` receives.
fn encode(frame: &DataFrame, ext: &str) -> Vec<u8> {
    let mut frame = frame.clone();
    let mut buf = Vec::new();
    match ext {
        "csv" => {
            CsvWriter::new(&mut buf).finish(&mut frame).unwrap();
        }
        "json" => {
            // NDJSON framing, which `parse_data` auto-detects (no leading `[`).
            JsonWriter::new(&mut buf)
                .with_json_format(JsonFormat::JsonLines)
                .finish(&mut frame)
                .unwrap();
        }
        "parquet" => {
            ParquetWriter::new(&mut buf).finish(&mut frame).unwrap();
        }
        "avro" => {
            polars::io::avro::AvroWriter::new(&mut buf)
                .finish(&mut frame)
                .unwrap();
        }
        other => panic!("unsupported bench format: {other}"),
    }
    buf
}

fn bench_parse_data(c: &mut Criterion) {
    let frame = sample_frame();
    let mut group = c.benchmark_group("parse_data");

    for ext in ["csv", "json", "parquet", "avro"] {
        let bytes = encode(&frame, ext);
        let file_name = format!("data.{ext}");
        group.throughput(Throughput::Bytes(bytes.len() as u64));
        group.bench_with_input(BenchmarkId::from_parameter(ext), &bytes, |b, bytes| {
            b.iter(|| {
                let frame = parse_data(black_box(bytes), &file_name).unwrap();
                black_box(frame.height())
            });
        });
    }

    group.finish();
}

/// Encode the dataset as a Vortex file in memory, mirroring the writer path used
/// by the `test_parse_data_vortex_round_trip` unit test. Two primitive columns
/// keep array construction simple (strings need a separate VarBin builder).
fn encode_vortex() -> Vec<u8> {
    use vortex::VortexSessionDefault;
    use vortex::array::IntoArray;
    use vortex::array::arrays::StructArray;
    use vortex::buffer::Buffer;
    use vortex::file::WriteOptionsSessionExt;
    use vortex::io::runtime::BlockingRuntime;
    use vortex::io::runtime::current::CurrentThreadRuntime;
    use vortex::io::session::RuntimeSessionExt;
    use vortex::session::VortexSession;

    let runtime = CurrentThreadRuntime::new();
    let session = <VortexSession as VortexSessionDefault>::default().with_handle(runtime.handle());

    let ids: Buffer<i64> = (0..ROWS as i64).collect();
    let values: Buffer<f64> = (0..ROWS).map(|i| i as f64 * 1.5).collect();
    let array =
        StructArray::from_fields(&[("id", ids.into_array()), ("value", values.into_array())])
            .expect("building the vortex struct array")
            .into_array();

    let mut bytes: Vec<u8> = Vec::new();
    session
        .write_options()
        .blocking(&runtime)
        .write(&mut bytes, array.to_array_iterator())
        .expect("writing the vortex payload");
    bytes
}

/// Benchmarks the Vortex decode path (Vortex is a first-class format here).
fn bench_parse_vortex(c: &mut Criterion) {
    let bytes = encode_vortex();
    // Same group name as the other formats so it lands under `parse_data/` too.
    let mut group = c.benchmark_group("parse_data");
    group.throughput(Throughput::Bytes(bytes.len() as u64));
    group.bench_with_input(BenchmarkId::from_parameter("vortex"), &bytes, |b, bytes| {
        b.iter(|| {
            let frame = parse_data(black_box(bytes), "data.vortex").unwrap();
            black_box(frame.height())
        });
    });
    group.finish();
}
