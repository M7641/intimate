//! Test tooling for the upload API: sample-data generation, a one-shot upload
//! smoke test, and a concurrent HTTP load benchmark. Exposed as the `gen-data`,
//! `test-upload` and `benchmark` subcommands of the `tako` binary (see main.rs).

use std::fs::{self, File};
use std::io::{BufWriter, Cursor};
use std::sync::Arc;
use std::time::{Duration, Instant};

use std::path::PathBuf;

use clap::ValueEnum;
use polars::io::avro::AvroWriter;
use polars::prelude::*;
use tokio::sync::Semaphore;

#[derive(Clone, Debug, ValueEnum)]
pub enum BenchFormat {
    Csv,
    Parquet,
    Json,
    Avro,
    All,
}

impl BenchFormat {
    fn formats(&self) -> Vec<&'static str> {
        match self {
            BenchFormat::Csv => vec!["csv"],
            BenchFormat::Parquet => vec!["parquet"],
            BenchFormat::Json => vec!["json"],
            BenchFormat::Avro => vec!["avro"],
            BenchFormat::All => vec!["csv", "parquet", "json", "avro"],
        }
    }
}

impl std::fmt::Display for BenchFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BenchFormat::Csv => write!(f, "csv"),
            BenchFormat::Parquet => write!(f, "parquet"),
            BenchFormat::Json => write!(f, "json"),
            BenchFormat::Avro => write!(f, "avro"),
            BenchFormat::All => write!(f, "all"),
        }
    }
}

// ── Shared DataFrame generator ──────────────────────────────────────────────

/// Single source of truth for synthetic sample data — see [`crate::fake`].
fn gen_sample_df(rows: usize) -> PolarsResult<DataFrame> {
    crate::fake::sample_frame(rows)
}

// ── gen_data (refactored to use gen_sample_df) ──────────────────────────────

pub fn default_output_dir() -> String {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    PathBuf::from(manifest_dir)
        .join("../../api_testing/sample_files")
        .to_string_lossy()
        .into_owned()
}

pub fn gen_data(output_dir: &str, rows: usize) -> anyhow::Result<()> {
    let dir = PathBuf::from(output_dir);
    fs::create_dir_all(&dir)?;

    let df = gen_sample_df(rows)?;

    // Parquet
    let path = dir.join("data.parquet");
    let file = File::create(&path)?;
    ParquetWriter::new(BufWriter::new(file)).finish(&mut df.clone())?;
    println!("Created {}", path.display());

    // CSV
    let path = dir.join("data.csv");
    let file = File::create(&path)?;
    CsvWriter::new(BufWriter::new(file)).finish(&mut df.clone())?;
    println!("Created {}", path.display());

    // JSON
    let path = dir.join("data.json");
    let file = File::create(&path)?;
    JsonWriter::new(BufWriter::new(file)).finish(&mut df.clone())?;
    println!("Created {}", path.display());

    // Avro
    let path = dir.join("data.avro");
    let file = File::create(&path)?;
    AvroWriter::new(BufWriter::new(file)).finish(&mut df.clone())?;
    println!("Created {}", path.display());

    Ok(())
}

// ── test_upload (unchanged) ─────────────────────────────────────────────────

pub async fn test_upload(base_url: &str, schema: &str, table: Option<&str>) -> anyhow::Result<()> {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let file_dir = PathBuf::from(manifest_dir).join("../../api_testing/sample_files");

    let files = ["data.parquet", "data.csv", "data.json", "data.avro"];
    let client = reqwest::Client::new();

    for filename in files {
        let filepath = file_dir.join(filename);
        let bytes = fs::read(&filepath)?;

        let part = reqwest::multipart::Part::bytes(bytes).file_name(filename.to_string());
        let form = reqwest::multipart::Form::new().part("file", part);

        // The server requires `table`, so derive one from the file name when the
        // caller didn't pass `--table` (the convenience now lives client-side).
        let base_name = filename
            .rsplit_once('.')
            .map(|(n, _)| n)
            .unwrap_or(filename);
        let table_name = table.unwrap_or(base_name);
        let url = format!("{base_url}/upload?schema={schema}&table={table_name}");
        match client.post(&url).multipart(form).send().await {
            Ok(response) => {
                let status = response.status();
                let body = response.text().await.unwrap_or_default();
                println!("Uploaded {filename}: Status {status}");
                println!("  {body}");
            }
            Err(e) => {
                println!("Failed to upload {filename}: {e}");
            }
        }
    }

    Ok(())
}

// ── Benchmark types ─────────────────────────────────────────────────────────

struct RequestResult {
    latency: Duration,
    status: u16,
    success: bool,
    payload_size: usize,
}

struct BenchmarkStats {
    total_duration: Duration,
    success_count: usize,
    fail_count: usize,
    requests_per_sec: f64,
    mb_per_sec: f64,
    p50: Duration,
    p95: Duration,
    p99: Duration,
    min: Duration,
    max: Duration,
    avg: Duration,
}

// ── Payload generation ──────────────────────────────────────────────────────

fn gen_data_bytes(rows: usize, format: &str) -> anyhow::Result<(Vec<u8>, String)> {
    let mut df = gen_sample_df(rows)?;
    let mut buf = Cursor::new(Vec::new());

    let filename = format!("bench.{format}");

    match format {
        "csv" => {
            CsvWriter::new(&mut buf).finish(&mut df)?;
        }
        "parquet" => {
            ParquetWriter::new(&mut buf).finish(&mut df)?;
        }
        "json" => {
            JsonWriter::new(&mut buf).finish(&mut df)?;
        }
        "avro" => {
            AvroWriter::new(&mut buf).finish(&mut df)?;
        }
        other => anyhow::bail!("unsupported format: {other}"),
    }

    Ok((buf.into_inner(), filename))
}

// ── Stats computation ───────────────────────────────────────────────────────

fn compute_stats(
    results: &[RequestResult],
    total_duration: Duration,
    total_bytes: usize,
) -> BenchmarkStats {
    let mut latencies: Vec<Duration> = results.iter().map(|r| r.latency).collect();
    latencies.sort();

    let n = latencies.len();
    let success_count = results.iter().filter(|r| r.success).count();
    let fail_count = n - success_count;

    let total_secs = total_duration.as_secs_f64();
    let requests_per_sec = n as f64 / total_secs;
    let mb_per_sec = (total_bytes as f64 / (1024.0 * 1024.0)) / total_secs;

    let sum: Duration = latencies.iter().sum();
    let avg = sum / n as u32;

    let percentile = |p: f64| -> Duration {
        let idx = ((p / 100.0) * n as f64).ceil() as usize;
        latencies[idx.min(n) - 1]
    };

    BenchmarkStats {
        total_duration,
        success_count,
        fail_count,
        requests_per_sec,
        mb_per_sec,
        p50: percentile(50.0),
        p95: percentile(95.0),
        p99: percentile(99.0),
        min: latencies[0],
        max: latencies[n - 1],
        avg,
    }
}

fn format_duration(d: Duration) -> String {
    let ms = d.as_secs_f64() * 1000.0;
    if ms >= 1000.0 {
        format!("{:.2}s", ms / 1000.0)
    } else {
        format!("{ms:.2}ms")
    }
}

fn print_stats(stats: &BenchmarkStats, label: &str, config_line: &str) {
    println!();
    println!("============================================================");
    println!("  Tako Upload Benchmark Results{label}");
    println!("============================================================");
    println!("  Config: {config_line}");
    println!("------------------------------------------------------------");
    println!(
        "  Total time:       {}",
        format_duration(stats.total_duration)
    );
    println!(
        "  Requests:         {} total, {} ok, {} failed",
        stats.success_count + stats.fail_count,
        stats.success_count,
        stats.fail_count,
    );
    println!("------------------------------------------------------------");
    println!("  Throughput:");
    println!("    Requests/sec:   {:.2}", stats.requests_per_sec);
    println!("    MB/sec:         {:.2}", stats.mb_per_sec);
    println!("------------------------------------------------------------");
    println!("  Latency:");
    println!("    min:            {}", format_duration(stats.min));
    println!("    avg:            {}", format_duration(stats.avg));
    println!("    p50:            {}", format_duration(stats.p50));
    println!("    p95:            {}", format_duration(stats.p95));
    println!("    p99:            {}", format_duration(stats.p99));
    println!("    max:            {}", format_duration(stats.max));
    println!("============================================================");
}

// ── Benchmark runner ────────────────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
pub async fn run_benchmark(
    base_url: &str,
    schema: &str,
    table: Option<&str>,
    concurrency: usize,
    total_requests: usize,
    format: &BenchFormat,
    rows: usize,
    warmup: usize,
) -> anyhow::Result<()> {
    let formats = format.formats();

    // Pre-generate payloads for each format
    let mut payloads: Vec<(Arc<Vec<u8>>, String)> = Vec::new();
    for fmt in &formats {
        let (bytes, filename) = gen_data_bytes(rows, fmt)?;
        println!("Generated {filename}: {} bytes ({rows} rows)", bytes.len());
        payloads.push((Arc::new(bytes), filename));
    }

    let client = reqwest::Client::new();
    // The server requires `table`; derive a fixed one from the first payload's
    // file name when the caller didn't pass `--table`.
    let derived = payloads[0]
        .1
        .rsplit_once('.')
        .map(|(n, _)| n)
        .unwrap_or(&payloads[0].1)
        .to_string();
    let table_name = table.unwrap_or(&derived);
    let url = format!("{base_url}/upload?schema={schema}&table={table_name}");

    // Warmup
    if warmup > 0 {
        println!("\nRunning {warmup} warmup requests...");
        for i in 0..warmup {
            let (ref data, ref filename) = payloads[i % payloads.len()];
            let part =
                reqwest::multipart::Part::bytes(data.as_ref().clone()).file_name(filename.clone());
            let form = reqwest::multipart::Form::new().part("file", part);
            let _ = client.post(&url).multipart(form).send().await;
        }
        println!("Warmup complete.");
    }

    println!("\nBenchmarking: {total_requests} requests, concurrency {concurrency}");
    println!("Formats: {format}");

    let semaphore = Arc::new(Semaphore::new(concurrency));
    let mut handles = Vec::with_capacity(total_requests);

    let bench_start = Instant::now();

    for i in 0..total_requests {
        let permit = semaphore.clone().acquire_owned().await?;
        let (ref data, ref filename) = payloads[i % payloads.len()];
        let data = Arc::clone(data);
        let filename = filename.clone();
        let client = client.clone();
        let url = url.clone();
        let payload_size = data.len();

        let handle = tokio::spawn(async move {
            let start = Instant::now();
            let part = reqwest::multipart::Part::bytes(data.as_ref().clone()).file_name(filename);
            let form = reqwest::multipart::Form::new().part("file", part);

            let result = match client.post(&url).multipart(form).send().await {
                Ok(resp) => {
                    let status = resp.status().as_u16();
                    RequestResult {
                        latency: start.elapsed(),
                        status,
                        success: (200..300).contains(&status),
                        payload_size,
                    }
                }
                Err(_) => RequestResult {
                    latency: start.elapsed(),
                    status: 0,
                    success: false,
                    payload_size,
                },
            };

            drop(permit);
            result
        });

        handles.push(handle);
    }

    // Collect results
    let mut results = Vec::with_capacity(total_requests);
    for handle in handles {
        results.push(handle.await?);
    }

    let total_duration = bench_start.elapsed();
    let total_bytes: usize = results.iter().map(|r| r.payload_size).sum();

    // Overall stats
    let stats = compute_stats(&results, total_duration, total_bytes);
    let config_line =
        format!("{total_requests} reqs, concurrency={concurrency}, format={format}, rows={rows}");
    print_stats(&stats, "", &config_line);

    // Per-format breakdown when using "all"
    if formats.len() > 1 {
        for (idx, fmt) in formats.iter().enumerate() {
            let fmt_results: Vec<&RequestResult> = results
                .iter()
                .enumerate()
                .filter(|(i, _)| i % payloads.len() == idx)
                .map(|(_, r)| r)
                .collect();

            if fmt_results.is_empty() {
                continue;
            }

            let fmt_bytes: usize = fmt_results.iter().map(|r| r.payload_size).sum();
            let owned: Vec<RequestResult> = fmt_results
                .iter()
                .map(|r| RequestResult {
                    latency: r.latency,
                    status: r.status,
                    success: r.success,
                    payload_size: r.payload_size,
                })
                .collect();

            let fmt_stats = compute_stats(&owned, total_duration, fmt_bytes);
            let label = format!(" [{fmt}]");
            let config = format!(
                "{} reqs, concurrency={concurrency}, format={fmt}, rows={rows}",
                owned.len()
            );
            print_stats(&fmt_stats, &label, &config);
        }
    }

    Ok(())
}
