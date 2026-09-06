# Testing in Rust

Practical examples for every testing approach. See [genisi.md](genisi.md) for the concepts behind each one.

Domain examples use a `Money` struct and the `flow` event store (Axum + SQLite).

## Contents

- [Unit Testing](#unit-testing)
- [Parameterized Testing — rstest / test-case](#parameterized-testing--rstest--test-case)
- [Property-Based Testing — proptest](#property-based-testing--proptest)
- [Snapshot Testing — insta](#snapshot-testing--insta)
- [Mutation Testing — cargo-mutants](#mutation-testing--cargo-mutants)
- [Fuzzing — cargo-fuzz](#fuzzing--cargo-fuzz)
- [Integration Testing — tests/ directory](#integration-testing--tests-directory)
- [Mocking — mockall](#mocking--mockall)
- [Doctests](#doctests)
- [Benchmarking — criterion / divan](#benchmarking--criterion--divan)
- [Contract Testing — proptest + oneshot](#contract-testing--proptest--oneshot)
- [Load Testing — oha](#load-testing--oha)
- [Dependencies](#dependencies)

---

## Unit Testing

Rust's test framework is built into the language. `#[test]` marks test functions, `#[cfg(test)]` conditionally compiles the test module. `assert_eq!` gives both values on failure; `assert!` checks a boolean; `#[should_panic]` asserts that a function panics.

```rust
// src/lib.rs
#[derive(Debug, Clone, PartialEq)]
struct Money {
    cents: i64,
    currency: &'static str,
}

impl Money {
    fn new(cents: i64, currency: &'static str) -> Self {
        Self { cents, currency }
    }

    fn add(&self, other: &Money) -> Result<Money, String> {
        if self.currency != other.currency {
            return Err(format!("cannot add {} to {}", self.currency, other.currency));
        }
        Ok(Money::new(self.cents + other.cents, self.currency))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_same_currency() {
        let a = Money::new(100, "USD");
        let b = Money::new(250, "USD");
        assert_eq!(a.add(&b).unwrap(), Money::new(350, "USD"));
    }

    #[test]
    fn add_zero() {
        let a = Money::new(500, "EUR");
        let zero = Money::new(0, "EUR");
        assert_eq!(a.add(&zero).unwrap(), a);
    }

    #[test]
    fn add_different_currency_fails() {
        let usd = Money::new(100, "USD");
        let eur = Money::new(100, "EUR");
        assert!(usd.add(&eur).is_err());
    }

    #[test]
    #[should_panic(expected = "out of range")]
    fn negative_overflow_panics() {
        Money::new(i64::MIN, "USD").add(&Money::new(-1, "USD"))
            .expect("out of range");
    }
}
```

### Unit vs integration test placement

| Location                           | Access          | Compiled when | Use case                                     |
| ---------------------------------- | --------------- | ------------- | -------------------------------------------- |
| `#[cfg(test)] mod tests` in source | Private items   | `cargo test`  | Unit tests — testing internal logic          |
| `tests/*.rs` directory             | Public API only | `cargo test`  | Integration tests — testing from the outside |

---

## Parameterized Testing — rstest / test-case

### rstest

The `rstest` crate adds `#[case]`-based parameterization. Each case becomes a named sub-test. Also supports fixtures for reusable setup.

```rust
use rstest::rstest;

#[rstest]
#[case(100, 250, 350)]
#[case(0, 500, 500)]
#[case(-100, 100, 0)]
#[case(i64::MAX - 1, 1, i64::MAX)]
fn test_money_addition(#[case] a: i64, #[case] b: i64, #[case] expected: i64) {
    let result = Money::new(a, "USD").add(&Money::new(b, "USD")).unwrap();
    assert_eq!(result.cents, expected);
}

// rstest also supports fixtures — reusable setup shared across tests
#[fixture]
fn test_db() -> SqlitePool {
    // ...set up in-memory database
}

#[rstest]
#[tokio::test]
async fn test_append_event(test_db: SqlitePool) {
    // test_db is automatically injected
}
```

### test-case

Lighter alternative — parameterized cases without fixtures. The return value becomes the assertion.

```rust
use test_case::test_case;

#[test_case("USD", "USD" => true  ; "same currency")]
#[test_case("USD", "EUR" => false ; "different currency")]
fn can_add(a: &str, b: &str) -> bool {
    Money::new(100, a).add(&Money::new(100, b)).is_ok()
}
```

---

## Property-Based Testing — proptest

`proptest` supports shrinking, custom strategies, and integrates with Rust's type system. The `proptest!` macro generates test cases from `Strategy` implementations.

### Roundtrip property

```rust
use proptest::prelude::*;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
struct Event {
    event_type: String,
    sequence: u64,
}

proptest! {
    #[test]
    fn json_roundtrip(
        event_type in "[a-zA-Z]{1,20}",
        sequence in 0u64..10000,
    ) {
        let event = Event { event_type, sequence };
        let json = serde_json::to_string(&event).unwrap();
        let decoded: Event = serde_json::from_str(&json).unwrap();
        prop_assert_eq!(event, decoded);
    }
}
```

### `prop_compose!` — domain-specific generators

```rust
use proptest::prelude::*;

prop_compose! {
    fn valid_event()(
        event_type in prop::string::string_regex("[A-Z][a-zA-Z]{2,15}").unwrap(),
        payload in prop::collection::hash_map("[a-z]+", "[a-zA-Z0-9 ]{0,50}", 0..5),
    ) -> serde_json::Value {
        serde_json::json!({
            "type": event_type,
            "payload": payload,
        })
    }
}

proptest! {
    #[test]
    fn all_valid_events_are_accepted(event in valid_event()) {
        let body = serde_json::to_string(&event).unwrap();
        let parsed: Result<EventInput, _> = serde_json::from_str(&body);
        prop_assert!(parsed.is_ok());
    }
}
```

### Invariant property — sequence numbers

```rust
proptest! {
    #[test]
    fn sequence_numbers_always_increase(
        events in prop::collection::vec(valid_event(), 1..20)
    ) {
        let mut store = EventStore::new();
        let mut last_seq = 0u64;

        for event in events {
            let appended = store.append("stream-1", event);
            prop_assert!(appended.sequence > last_seq);
            last_seq = appended.sequence;
        }
    }
}
```

### Configuration

```rust
// Override at the test level
proptest! {
    #![proptest_config(ProptestConfig::with_cases(1000))]

    #[test]
    fn intensive_test(x in 0i64..1000) {
        prop_assert!(x >= 0);
    }
}

// Or via environment variable:
// PROPTEST_CASES=10000 cargo test

// Failed cases are saved to .proptest-regressions files
// and replayed automatically on subsequent runs.
```

### Key strategies

| Strategy                                       | Generates                                           |
| ---------------------------------------------- | --------------------------------------------------- |
| `any::<T>()`                                   | Any value of type T (if `Arbitrary` is implemented) |
| `0i64..1000`                                   | Integer range                                       |
| `"[a-z]{1,10}"`                                | String matching regex                               |
| `prop::collection::vec(strat, 0..10)`          | Vec of generated values                             |
| `prop::collection::hash_map(k, v, 0..5)`       | HashMap                                             |
| `prop::option::of(strat)`                      | Option<T>                                           |
| `prop_oneof![strat1, strat2]`                  | One of several strategies                           |
| `strat.prop_filter("reason", \|v\| predicate)` | Filtered values                                     |
| `strat.prop_flat_map(\|v\| dependent_strat)`   | Dependent generation                                |

---

## Snapshot Testing — insta

```rust
use insta::{assert_json_snapshot, assert_snapshot};

#[test]
fn health_response() {
    let response = get_health();
    assert_snapshot!(response, @"ok");
    // Inline snapshots: the expected value lives right in the source.
    // `cargo insta review` updates them interactively.
}

#[test]
fn event_response_shape() {
    let event = append_event("order-1", "OrderCreated", json!({"item": "Widget"}));
    assert_json_snapshot!(event, {
        ".event_id" => "[uuid]",        // redact dynamic fields
        ".timestamp" => "[timestamp]",
    }, @r###"
    {
      "event_id": "[uuid]",
      "stream_id": "order-1",
      "sequence_number": 1,
      "event_type": "OrderCreated",
      "data": {
        "item": "Widget"
      },
      "timestamp": "[timestamp]"
    }
    "###);
}
```

The `cargo insta review` workflow lets you accept/reject snapshot changes interactively. The `{".field" => "[redaction]"}` syntax handles non-deterministic values (UUIDs, timestamps).

```bash
cargo insta test        # run tests, save pending snapshots
cargo insta review      # interactively accept/reject changes
cargo insta test --review  # both in one step
```

---

## Mutation Testing — cargo-mutants

```bash
cargo install cargo-mutants
cargo mutants
# or target a single file:
cargo mutants --file src/lib.rs
```

Output shows which mutations survived (missed by tests), which were caught (killed), and which timed out. A surviving mutant like `replaced + with -` in `Money::add` means your test suite doesn't sufficiently exercise addition.

```bash
# Faster: only test mutations in functions matching a pattern
cargo mutants --re "Money::add"

# Skip slow tests
cargo mutants --timeout 30
```

---

## Fuzzing — cargo-fuzz

```bash
cargo install cargo-fuzz
cargo fuzz init
# creates fuzz/ directory with fuzz_targets/
```

```rust
// fuzz/fuzz_targets/fuzz_event_parse.rs
#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(text) = std::str::from_utf8(data) {
        // Should never panic on any input
        let _ = serde_json::from_str::<EventInput>(text);
    }
});
```

```bash
cargo fuzz run fuzz_event_parse -- -max_total_time=60
# Any crash is saved to fuzz/artifacts/ — turn it into a regression test
```

When the fuzzer finds a crash, the input is saved to `fuzz/artifacts/`. Convert it to a unit test to prevent regressions.

---

## Integration Testing — tests/ directory

Rust has a clear separation: `#[cfg(test)] mod tests` for unit tests (same file, private access), `tests/` directory for integration tests (separate crate, public API only). The `lib.rs + main.rs` pattern exposes types to integration tests via `use your_crate::`.

The flow project's `tests/integration.rs` demonstrates the canonical pattern:

```rust
// tests/integration.rs
use axum::body::Body;
use axum::http::{Request, StatusCode};
use tower::ServiceExt;  // for .oneshot()

async fn build_test_app() -> Router {
    let db = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("in-memory db");

    // ... create tables, build state, return Router
}

#[tokio::test]
async fn health_check() {
    let app = build_test_app().await;
    let response = app
        .oneshot(Request::get("/health").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn separate_streams_have_independent_sequences() {
    let app = build_test_app().await;
    // Post to stream A, post to stream B
    // Assert stream B starts at sequence 1, independent of A
}
```

### Key patterns

- `tower::ServiceExt::oneshot()` — test Axum apps without binding a port
- `sqlite::memory:` — fresh database per test, no cleanup needed
- `build_test_app()` — factory function isolates setup from assertions
- `http-body-util::BodyExt::collect()` — read response bodies in tests

---

## Mocking — mockall

In Rust, mocking is trait-based. Define a trait for the dependency, implement it for production, and generate a mock with `#[automock]`.

```rust
use mockall::{automock, predicate::*};

#[automock]
trait Notifier {
    fn send(&self, to: &str, subject: &str) -> Result<(), String>;
}

// Production code uses `impl Notifier`
fn complete_order(order_id: u64, notifier: &impl Notifier) -> Result<(), String> {
    // ... process order ...
    notifier.send("customer@example.com", &format!("Order #{order_id} Complete"))
}

#[test]
fn order_completion_sends_notification() {
    let mut mock = MockNotifier::new();
    mock.expect_send()
        .with(eq("customer@example.com"), eq("Order #42 Complete"))
        .times(1)
        .returning(|_, _| Ok(()));

    complete_order(42, &mock).unwrap();
    // mock verifies expectations on drop
}
```

The alternative is often to avoid mocking entirely: use `sqlite::memory:` instead of mocking the database, use `tower::ServiceExt::oneshot()` instead of mocking HTTP clients. Real implementations with in-memory backends are less brittle than mocks.

---

## Doctests

Rust doc tests compile and run as integration tests. They are real code, not string matching. Lines prefixed with `# ` are compiled but hidden from rendered docs.

````rust
/// Adds two amounts in the same currency.
///
/// # Examples
///
/// ```
/// # use mylib::Money;
/// let a = Money::new(100, "USD");
/// let b = Money::new(250, "USD");
/// let sum = a.add(&b).unwrap();
/// assert_eq!(sum.cents, 350);
/// ```
///
/// Different currencies produce an error:
///
/// ```
/// # use mylib::Money;
/// let usd = Money::new(100, "USD");
/// let eur = Money::new(100, "EUR");
/// assert!(usd.add(&eur).is_err());
/// ```
pub fn add(&self, other: &Money) -> Result<Money, String> {
    // ...
}
````

```bash
cargo test --doc
```

---

## Benchmarking — criterion / divan

### criterion

The standard. HTML reports with statistical analysis, regression detection across runs.

```toml
# Cargo.toml
[dev-dependencies]
criterion = { version = "0.5", features = ["html_reports"] }

[[bench]]
name = "my_benchmarks"
harness = false
```

```rust
// benches/my_benchmarks.rs
use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};

fn bench_money_addition(c: &mut Criterion) {
    let a = Money::new(100, "USD");
    let b = Money::new(250, "USD");

    c.bench_function("money_add", |bencher| {
        bencher.iter(|| {
            black_box(a.add(&b).unwrap())
            // black_box prevents the compiler from optimizing away the result
        })
    });
}

// Parameterized benchmarks — vary the input size
fn bench_event_store_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("event_store_append");

    for size in [10, 100, 1000, 5000] {
        group.bench_with_input(
            BenchmarkId::from_parameter(size),
            &size,
            |b, &size| {
                b.iter(|| {
                    let mut store = EventStore::new();
                    for i in 0..size {
                        store.append("stream-1", Event::new(format!("Type{i}")));
                    }
                })
            },
        );
    }
    group.finish();
}

criterion_group!(benches, bench_money_addition, bench_event_store_scaling);
criterion_main!(benches);
```

```bash
cargo bench
# Results in target/criterion/ — open report/index.html for graphs

# Compare against a baseline
cargo bench -- --save-baseline main
# ... make changes ...
cargo bench -- --baseline main
```

### divan

Newer alternative — less setup, attribute-based. No macros, no groups to define.

```toml
# Cargo.toml
[dev-dependencies]
divan = "0.1"

[[bench]]
name = "my_bench"
harness = false
```

```rust
// benches/my_bench.rs
fn main() {
    divan::main();
}

#[divan::bench]
fn money_add() -> Money {
    Money::new(100, "USD").add(&Money::new(250, "USD")).unwrap()
}

#[divan::bench(args = [10, 100, 1000])]
fn event_store_append(n: usize) {
    let mut store = EventStore::new();
    for i in 0..n {
        store.append("stream-1", Event::new(format!("Type{i}")));
    }
}
```

---

## Contract Testing — proptest + oneshot

Rust doesn't have a Schemathesis equivalent, but you can achieve similar results by combining `proptest` with `tower::ServiceExt::oneshot()` — generate arbitrary request bodies and verify the server always responds correctly.

```rust
use proptest::prelude::*;

prop_compose! {
    fn arbitrary_event_body()(
        event_type in "[A-Z][a-z]{2,10}(Created|Updated|Deleted)",
        has_payload in any::<bool>(),
        payload_value in "[a-zA-Z0-9 ]{0,100}",
    ) -> String {
        if has_payload {
            format!(r#"{{"type":"{event_type}","payload":{{"data":"{payload_value}"}}}}"#)
        } else {
            format!(r#"{{"type":"{event_type}"}}"#)
        }
    }
}

proptest! {
    #[test]
    fn server_never_500s(body in arbitrary_event_body()) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let app = build_test_app().await;
            let response = app
                .oneshot(
                    Request::post("/streams/test/events")
                        .header("Content-Type", "application/json")
                        .body(Body::from(body))
                        .unwrap(),
                )
                .await
                .unwrap();

            // The server should never return 500 — only 201, 400, or 422
            prop_assert!(response.status() != StatusCode::INTERNAL_SERVER_ERROR);
        });
    }
}
```

---

## Load Testing — oha

[oha](https://github.com/hatoo/oha) is a Rust-native HTTP load generator built on tokio and hyper. Think of it as a modern, TUI-equipped replacement for `ab` or `hey` — single binary, real-time terminal UI with latency histograms and status code distribution, and HTTP/2 support out of the box.

```bash
cargo install oha
# or: brew install oha

# Basic: 200 requests, 50 concurrent connections
oha -n 200 -c 50 http://localhost:8000/health

# Sustained: run for 30 seconds
oha -z 30s -c 100 http://localhost:8000/streams/order-1/events

# POST with body
oha -z 10s -c 20 -m POST \
  -H "Content-Type: application/json" \
  -d '{"type":"OrderCreated","payload":{"item":"Widget"}}' \
  http://localhost:8000/streams/load-test/events

# HTTP/2 (useful for testing multiplexing behavior)
oha -z 15s -c 50 --http2 http://localhost:8000/health

# Disable TUI for CI — outputs summary stats
oha -z 10s -c 50 --no-tui http://localhost:8000/health

# JSON output for scripting
oha -z 10s -c 50 --no-tui -j http://localhost:8000/health > results.json
```

The TUI shows live request rate, latency percentiles (p50/p90/p99), status code breakdown, and a latency histogram — all updating in real time during the run. For CI, `--no-tui -j` gives machine-readable JSON output.

Compared to other load testing tools:

| Tool       | Language | Strengths                                                    |
| ---------- | -------- | ------------------------------------------------------------ |
| **oha**    | Rust     | Single binary, real-time TUI, HTTP/2, very low overhead      |
| **drill**  | Rust     | YAML-based scenarios, multiple endpoints in one run          |
| **goose**  | Rust     | Programmable user behavior (like Locust), highest throughput |
| **k6**     | Go       | JavaScript scripting, cloud integration, CI-friendly         |
| **locust** | Python   | Python scripting, web UI, easiest to customize               |

oha is the best choice when you want a quick, zero-config load test from the command line — no scripts, no config files, just point and shoot.

---

## Dependencies

```toml
# Cargo.toml
[dev-dependencies]
proptest = "1"
rstest = "0.23"
insta = { version = "1", features = ["json", "yaml"] }
mockall = "0.13"
criterion = { version = "0.5", features = ["html_reports"] }
divan = "0.1"

# Integration testing for Axum
tower = { version = "0.5", features = ["util"] }
http-body-util = "0.1"

# CLI tools (not in Cargo.toml):
# cargo install cargo-mutants cargo-fuzz
```
