# Integration tests against a real backing service (Rust)

Some tests can't run against an in-memory stub — they need a *real* Postgres
(server-side SQL the code actually issues, `COPY`, window functions, real fixtures)
or a *real* S3 API (object put/get/list). For those, **spin up a throwaway
container, seed it, assert, drop it.**

Standard: the **`testcontainers` crate** (dev-dep). It manages the container
lifecycle from inside the test process and tears it down on drop — no manual
`docker stop`. This is the one place we accept a container SDK rather than shelling
out: testcontainers-rs is the idiomatic, well-maintained Rust path and avoids
hand-rolling process management in every crate.

Two rules keep these a clean PR gate instead of a flaky one:

- **Skip when Docker is absent**, so the suite stays green locally and the gate
  only bites in CI (where Docker is present). These live in `tests/*_it.rs` and
  run under the normal inherited `test` task — no separate moon task needed.
- **Reuse, don't restart per test.** Container startup dominates; if a module has
  many cases, start one container in a shared harness and TRUNCATE + re-seed
  between cases rather than booting one per `#[test]`.

CI: `ubuntu-latest` has Docker, and the workflow installs `libpq-dev` (needed to
build the `postgres` crate), so these run under the existing affected `moon ci`
`test` task. Pin the image tag in the module config for reproducibility.

```toml
# Cargo.toml — workspace dev-deps
[dev-dependencies]
testcontainers = "0.x"
testcontainers-modules = { version = "0.x", features = ["postgres"] }
postgres = { version = "0.19", default-features = false }   # native sync client
aws-config = "1"
aws-sdk-s3 = "1"
```

## Disposable Postgres (data-backed tests)

Use the Postgres module from `testcontainers-modules`. **Seed from versioned `.sql`
files** — schema then fixtures, in filename order (`tests/seeds/01_schema.sql`,
`02_fixtures.sql`). Sharing the seed SQL with a Python suite keeps one source of
truth for the fixtures.

```rust
// tests/events_it.rs   (integration test — public API only)
use testcontainers::runners::SyncRunner;
use testcontainers_modules::postgres::Postgres as PgImage;

fn docker_available() -> bool {
    std::process::Command::new("docker").arg("info").output()
        .map(|o| o.status.success()).unwrap_or(false)
}

fn apply_seeds(client: &mut postgres::Client) {
    // include_str! bakes the SQL into the test binary, so there's no runtime path
    // dependence and the files are tracked as build inputs.
    for sql in [
        include_str!("seeds/01_schema.sql"),
        include_str!("seeds/02_fixtures.sql"),
    ] {
        client.batch_execute(sql).expect("seed failed");
    }
}

#[test]
fn regional_totals() {
    if !docker_available() { eprintln!("skipping: Docker unavailable"); return; }

    let node = PgImage::default().start().expect("start postgres");
    let port = node.get_host_port_ipv4(5432).expect("port");
    let dsn = format!(
        "host=localhost port={port} user=postgres password=postgres dbname=postgres"
    );
    let mut client = postgres::Client::connect(&dsn, postgres::NoTls).unwrap();

    apply_seeds(&mut client);

    let rows = client
        .query("SELECT region, SUM(amount)::int8 FROM events GROUP BY region ORDER BY 1", &[])
        .unwrap();
    let got: Vec<(String, i64)> = rows.iter().map(|r| (r.get(0), r.get(1))).collect();
    assert_eq!(got, vec![("amer".into(), 80), ("emea".into(), 170)]);
    // `node` drops here → container is removed automatically.
}
```

For a module with many cases, build the container + a connection once in a shared
helper and `TRUNCATE ... RESTART IDENTITY CASCADE` + re-apply fixtures between
cases, rather than `start()`-ing a container per `#[test]`.

## Mocking S3 — S3Mock container

`adobe/s3mock` (Apache-2.0) is a lightweight, S3-only mock — much smaller than a
full LocalStack when all you need is the object API. There's no dedicated
testcontainers module, so run it via `GenericImage`. It serves the S3 API on port
**9090** (HTTP), pre-creates buckets from a comma-separated env var, and only
supports **path-style** access (`http://localhost:9090/bucket/key`) — so the
`aws-sdk-s3` client must set `force_path_style(true)`.

```rust
// tests/storage_it.rs
use testcontainers::core::{IntoContainerPort, WaitFor};
use testcontainers::{GenericImage, ImageExt, runners::AsyncRunner};

#[tokio::test]
async fn put_then_list() {
    if !docker_available() { eprintln!("skipping: Docker unavailable"); return; }

    let node = GenericImage::new("adobe/s3mock", "latest")
        .with_exposed_port(9090.tcp())
        .with_wait_for(WaitFor::message_on_stdout("Started S3MockApplication"))
        .with_env_var("COM_ADOBE_TESTING_S3MOCK_STORE_INITIAL_BUCKETS", "test-bucket")
        .start().await.expect("start s3mock");
    let port = node.get_host_port_ipv4(9090).await.expect("port");

    let conf = aws_config::from_env()
        .endpoint_url(format!("http://localhost:{port}"))
        .region(aws_config::Region::new("us-east-1"))
        .credentials_provider(aws_sdk_s3::config::Credentials::new(
            "test", "test", None, None, "test"))
        .load().await;
    let s3 = aws_sdk_s3::Client::from_conf(
        aws_sdk_s3::config::Builder::from(&conf).force_path_style(true).build());

    s3.put_object().bucket("test-bucket").key("events/1.json")
        .body(b"{}".to_vec().into()).send().await.unwrap();
    let listed = s3.list_objects_v2().bucket("test-bucket").send().await.unwrap();
    let keys: Vec<_> = listed.contents().iter().filter_map(|o| o.key()).collect();
    assert_eq!(keys, ["events/1.json"]);
    // `node` drops here → container is removed automatically.
}
```

Pin `adobe/s3mock` to a digest rather than `latest` for a reproducible gate. If a
test needs isolation from others' objects, give it its own bucket (extend the
`COM_ADOBE_TESTING_S3MOCK_STORE_INITIAL_BUCKETS` list or `create_bucket`).
