//! Black-box integration test of the *production container image*.
//!
//! The scratch image has no shell, so it can only be exercised over HTTP —
//! exactly as production runs it. `testcontainers` manages the container
//! lifecycle and removes it on drop, so there is no manual teardown (contrast
//! the hand-rolled `trap` in `scripts/smoke-image.sh`).
//!
//! Prerequisites: the image must be built (`moon run nix_stack:image-build`) and
//! a container engine must be reachable. When no engine is available the test
//! skips cleanly, so it never fails a machine without Podman/Docker.
//!
//! With Podman, point testcontainers at the engine before running — the
//! `scripts/image-test.sh` wrapper (invoked by `moon run nix_stack:image-test`)
//! sets `DOCKER_HOST`, `TESTCONTAINERS_RYUK_DISABLED` and `IMAGE_REF` for you.

use testcontainers::core::{IntoContainerPort, WaitFor};
use testcontainers::runners::AsyncRunner;
use testcontainers::GenericImage;

/// True when a container engine responds to `info` — mirrors the skip guard used
/// by the Postgres/S3 integration tests so the suite stays green without Docker.
fn engine_available() -> bool {
    ["podman", "docker"].iter().any(|engine| {
        std::process::Command::new(engine)
            .arg("info")
            .output()
            .is_ok_and(|o| o.status.success())
    })
}

#[tokio::test]
async fn image_serves_health_info_and_frontend() {
    if !engine_available() {
        eprintln!("skipping: no container engine (podman/docker) reachable");
        return;
    }

    // Podman requires fully-qualified names, so the wrapper passes
    // IMAGE_REF=localhost/nix-stack:prod; the bare default suits Docker.
    let image_ref = std::env::var("IMAGE_REF").unwrap_or_else(|_| "nix-stack:prod".into());
    let (name, tag) = image_ref
        .rsplit_once(':')
        .unwrap_or((image_ref.as_str(), "latest"));

    let container = GenericImage::new(name, tag)
        .with_exposed_port(3000.tcp())
        // The backend logs this line to stdout once it is bound and serving.
        .with_wait_for(WaitFor::message_on_stdout("listening on"))
        .start()
        .await
        .expect("failed to start container");

    let port = container
        .get_host_port_ipv4(3000.tcp())
        .await
        .expect("no mapped host port for 3000");
    let base = format!("http://127.0.0.1:{port}");

    for (path, what) in [
        ("/api/health", "health endpoint"),
        ("/api/info", "info endpoint"),
        ("/", "static frontend (STATIC_DIR)"),
    ] {
        let resp = reqwest::get(format!("{base}{path}"))
            .await
            .unwrap_or_else(|e| panic!("request to {what} failed: {e}"));
        assert_eq!(resp.status(), 200, "{what} at {path} should return 200");
    }
    // `container` drops here → testcontainers removes it automatically.
}
