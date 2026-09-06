//! Shared helpers for the data_view integration tests.
//!
//! The pattern: boot one disposable Postgres container, seed it, then run the
//! real `data_view` binary against it on a free port and exercise the HTTP API.
//! Everything tears down on drop. Works against Docker or a Docker-compatible
//! Podman socket (`DOCKER_HOST` + `TESTCONTAINERS_RYUK_DISABLED=true`).

use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

pub mod snowflake_mock;

/// Whether a Docker (or Docker-compatible) engine answers `docker info`.
pub fn docker_available() -> bool {
    Command::new("docker")
        .arg("info")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Hard-fail (never skip) when no container engine is reachable. Skipping would
/// let a run with no Docker/Podman look like a green pass — these tests would
/// then silently cover nothing. Better to fail loudly with how to fix it.
pub fn require_container_engine() {
    const HINT: &str = "no container engine reachable — these integration tests need Docker or a \
Docker-compatible Podman socket.\n\
  • Docker: start Docker Desktop.\n\
  • Podman: `podman machine start`, then point Docker tooling at its socket:\n\
      export DOCKER_HOST=\"unix://$(podman machine inspect --format '{{.ConnectionInfo.PodmanSocket.Path}}')\"\n\
      export TESTCONTAINERS_RYUK_DISABLED=true";
    assert!(docker_available(), "{}", HINT);
}

/// Seed SQL, baked into the test binary so there's no runtime path dependence.
/// Order matters: schema, then fixtures, then dialect shims.
pub const SEED_SQL: [&str; 3] = [
    include_str!("../seeds/01_schema.sql"),
    include_str!("../seeds/02_fixtures.sql"),
    include_str!("../seeds/03_redshift_compat.sql"),
];

/// Apply the seed files to a freshly started Postgres container.
pub fn seed(dsn: &str) {
    let mut client =
        postgres::Client::connect(dsn, postgres::NoTls).expect("connect to seed Postgres");
    for sql in SEED_SQL {
        client.batch_execute(sql).expect("seed batch failed");
    }
}

/// Ask the OS for a free TCP port by binding to :0 and releasing it. A small
/// TOCTOU window exists before the server binds, but it's good enough for tests.
pub fn free_port() -> u16 {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind ephemeral port");
    listener.local_addr().expect("local_addr").port()
}

/// A running `data_view serve` process. Killed and reaped on drop.
pub struct Server {
    child: Child,
    pub base_url: String,
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Launch the real server binary on `port` with the given extra env vars, then
/// block until `/health/ready` answers 200.
pub fn start_server(port: u16, envs: &[(&str, String)]) -> Server {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_data_view"));
    cmd.arg("serve")
        .env("DATA_VIEW_PORT", port.to_string())
        .env("RUST_LOG", "warn")
        // No bundle in the test working dir: serve API only, no SPA noise.
        .env_remove("DATA_VIEW_STATIC_DIR");
    for (k, v) in envs {
        cmd.env(k, v);
    }
    let child = cmd.spawn().expect("spawn data_view serve");
    let base_url = format!("http://127.0.0.1:{port}");
    wait_ready(&base_url);
    Server { child, base_url }
}

fn wait_ready(base_url: &str) {
    let url = format!("{base_url}/health/ready");
    let client = reqwest::blocking::Client::new();
    let deadline = Instant::now() + Duration::from_secs(30);
    while Instant::now() < deadline {
        let ok = client
            .get(&url)
            .send()
            .map(|r| r.status().is_success())
            .unwrap_or(false);
        if ok {
            return;
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    panic!("server at {base_url} not ready within 30s");
}

/// GET a JSON endpoint, asserting a 200, and return the parsed body.
pub fn get_json(base_url: &str, path: &str) -> serde_json::Value {
    let client = reqwest::blocking::Client::new();
    let url = format!("{base_url}{path}");
    let resp = client
        .get(&url)
        .send()
        .unwrap_or_else(|e| panic!("GET {url}: {e}"));
    let status = resp.status();
    let body = resp.text().unwrap_or_default();
    assert!(status.is_success(), "GET {url} -> {status}: {body}");
    serde_json::from_str(&body).unwrap_or_else(|e| panic!("GET {url} bad JSON ({e}): {body}"))
}
