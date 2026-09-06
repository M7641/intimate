# tako test-tooling examples

Subcommands of the `tako` binary for generating sample data and testing the
upload API.

## Generate sample data

Generate 100 rows (default) of sample data in Parquet, CSV, JSON, and Avro formats:

```bash
cargo run -p tako -- gen-data
```

Custom row count and output directory:

```bash
cargo run -p tako -- gen-data --rows 1000 --output-dir ./my_data
```

## Test upload

Upload generated sample files to a running Tako API (defaults to `http://localhost:3000`):

```bash
cargo run -p tako -- test-upload
```

Against a different host or schema:

```bash
cargo run -p tako -- test-upload --base-url http://localhost:8080 --schema my_schema
```

## Benchmark

Run a basic throughput benchmark against a running Tako API (100 requests, 10 concurrent):

```bash
cargo run -p tako -- benchmark
```

Customize request count and concurrency:

```bash
cargo run -p tako -- benchmark --requests 200 --concurrency 20
```

Benchmark a specific format:

```bash
cargo run -p tako -- benchmark --format parquet --requests 1000 --concurrency 10
cargo run -p tako -- benchmark --format json --requests 1000 --concurrency 10
cargo run -p tako -- benchmark --format avro  --requests 1000 --concurrency 10
```

Benchmark all supported formats (csv, parquet, json, avro) with a per-format breakdown:

```bash
cargo run -p tako -- benchmark --format all --requests 40
```

Larger payloads with more rows per file:

```bash
cargo run -p tako -- benchmark --rows 10000 --requests 10
```

Against a different host, schema, or with custom warmup:

```bash
cargo run -p tako -- benchmark --base-url http://localhost:8080 --schema my_schema --warmup 10
```

Skip warmup entirely:

```bash
cargo run -p tako -- benchmark --warmup 0 --requests 50
```

## Full workflow

Generate data then upload it:

```bash
cargo run -p tako -- gen-data
cargo run -p tako -- test-upload
```

## Help

```bash
cargo run -p tako -- --help
cargo run -p tako -- gen-data --help
cargo run -p tako -- test-upload --help
cargo run -p tako -- benchmark --help
```
