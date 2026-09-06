//! Runtime axis (Rust): black-box integration test of the *production container
//! image*. Copy to `<crate>/tests/image_it.rs` and edit the port, wait line, and
//! asserted endpoints to match your image's contract.
//!
//! A lean/scratch image has no shell, so it can only be exercised over HTTP —
//! exactly as production runs it. `testcontainers` manages the container
//! lifecycle and removes it on drop, so there is no manual teardown.
//!
//! Dev-deps to add:
//!   testcontainers = { version = "*", features = ["blocking"] }  # or async
//!   tokio    = { version = "*", features = ["macros", "rt-multi-thread"] }
//!   reqwest  = "*"
//!
//! Prerequisites: the image must be built (`moon run <project>:image-build`) and
//! a container engine must be reachable. When none is, the test skips cleanly, so
//! it never fails a machine without Podman/Docker. The `moon run
//! <project>:image-test` wrapper sets DOCKER_HOST, TESTCONTAINERS_RYUK_DISABLED
//! and IMAGE_REF for you (see templates/moon-image-tasks.yml).

use testcontainers::core::{IntoContainerPort, WaitFor};
use testcontainers::runners::AsyncRunner;
use testcontainers::GenericImage;

/// True when a container engine responds to `info` — the same skip guard our
/// Postgres/S3 integration tests use, so the suite stays green without an engine.
fn engine_available() -> bool {
    ["podman", "docker"].iter().any(|engine| {
        std::process::Command::new(engine)
            .arg("info")
            .output()
            .is_ok_and(|o| o.status.success())
    })
}

#[tokio::test]
async fn image_serves_its_endpoints() {
    if !engine_available() {
        eprintln!("skipping: no container engine (podman/docker) reachable");
        return;
    }

    // Podman requires fully-qualified names, so the wrapper passes
    // IMAGE_REF=localhost/myapp:prod; the bare default suits Docker.
    let image_ref = std::env::var("IMAGE_REF").unwrap_or_else(|_| "myapp:prod".into());
    let (name, tag) = image_ref
        .rsplit_once(':')
        .unwrap_or((image_ref.as_str(), "latest"));

    let container = GenericImage::new(name, tag)
        .with_exposed_port(3000.tcp()) // EDIT: your exposed port
        // EDIT: the exact line the app logs to stdout once bound and serving.
        .with_wait_for(WaitFor::message_on_stdout("listening on"))
        .start()
        .await
        .expect("failed to start container");

    let port = container
        .get_host_port_ipv4(3000.tcp())
        .await
        .expect("no mapped host port");
    let base = format!("http://127.0.0.1:{port}");

    // EDIT: the endpoints that define your image's contract.
    for (path, what) in [
        ("/api/health", "health endpoint"),
        ("/", "static frontend / root"),
    ] {
        let resp = reqwest::get(format!("{base}{path}"))
            .await
            .unwrap_or_else(|e| panic!("request to {what} failed: {e}"));
        assert_eq!(resp.status(), 200, "{what} at {path} should return 200");
    }
    // `container` drops here → testcontainers removes it automatically.
}
