# Testing a container image — the three axes

A design note from the `nix_stack` pilot on how we test a production container
image, and how the same approach carries over to a Python service.

## The shift: test the artifact you ship

We deploy a container. So rather than aligning the dev environment with prod
byte-for-byte, we do the reverse: **build the prod image and test _it_, locally,
before it deploys.** The container you exercise is the container that runs in
production — not a stand-in with different libraries, a different entrypoint, or
a different user.

That reframes "test the app" into "test the image". And an image can be wrong in
three independent ways, so there are **three axes** of testing — each answering a
different question, each with the tool native to it. Forcing all three through a
single `bash` + `curl` script (where this pilot started) only covers the first,
and imperatively.

## The three axes

| Axis          | Question                           | Tool (here)              | Language-specific? |
| ------------- | ---------------------------------- | ------------------------ | ------------------ |
| **Runtime**   | Does it behave when containerised? | testcontainers           | **yes**            |
| **Structure** | Is the image assembled correctly?  | container-structure-test | no                 |
| **Hygiene**   | Is it safe and lean?               | Trivy                    | no                 |

The key property: **only the runtime axis depends on the language.** Structure
and hygiene operate on the _built image_ — its files, its metadata, its
vulnerabilities — which look the same whether the binary inside was written in
Rust, Python, or Go. This is what makes the approach portable (see [Doing this
for Python](#doing-this-for-python)).

### Runtime — does it behave?

Start the real container, drive it over HTTP, assert the responses. Because our
image is `scratch` (no shell), it can _only_ be tested from the outside — which
is a feature: we exercise it exactly as production does.

The tool is **testcontainers**, which manages the container lifecycle from inside
the test process and removes it automatically when the test ends. No manual
teardown, no leaked containers. The test lives next to the code
(`backend/tests/image_it.rs`) and runs under the normal test runner:

```rust
let container = GenericImage::new("nix-stack", "prod")
    .with_exposed_port(3000.tcp())
    .with_wait_for(WaitFor::message_on_stdout("listening on"))
    .start().await.expect("failed to start container");

let port = container.get_host_port_ipv4(3000.tcp()).await.unwrap();
// ... assert /api/health, /api/info, / all return 200 ...
// `container` drops here → testcontainers removes it automatically.
```

It skips cleanly when no container engine is reachable, so it never fails a
machine without Podman/Docker.

### Structure — is it assembled correctly?

Assert on the image's _shape_ without running the workload: which files exist,
and what the metadata says (user, exposed ports, entrypoint, env). This catches a
Dockerfile refactor that drops the non-root user or mislays the static assets —
things a runtime test might not notice.

The tool is **container-structure-test**, driven by a declarative YAML
(`structure-test.yaml`):

```yaml
fileExistenceTests:
  - {
      name: "backend binary present",
      path: "/nix-stack-backend",
      shouldExist: true,
    }
  - {
      name: "frontend index present",
      path: "/dist/index.html",
      shouldExist: true,
    }
metadataTest:
  user: "1000:1000"
  exposedPorts: ["3000"]
  entrypoint: ["/nix-stack-backend"]
  envVars:
    - { key: "STATIC_DIR", value: "/dist" }
```

### Hygiene — is it safe?

Scan the image for known vulnerabilities and fail the build on anything serious.
The tool is **Trivy**, gated on HIGH/CRITICAL severity. On a `scratch` image the
result is near-empty — which is itself the argument for `scratch`: nothing to
scan means almost nothing to exploit. Hygiene is where a minimal base image pays
off as a _number_, not a claim.

## How it runs — one command per axis, one for all three

Everything is wired as [moon](https://moonrepo.dev) tasks (`moon.yml`). Each test
task depends on `image-build`, so you can never test a stale image — moon rebuilds
it first.

```bash
moon run nix_stack:image-test       # runtime  (testcontainers)
moon run nix_stack:image-structure  # structure (container-structure-test)
moon run nix_stack:image-scan       # hygiene   (Trivy)
moon run nix_stack:image-check      # all three
```

Two implementation choices worth recording:

- **The `podman save` tarball is the pivot.** Structure and hygiene run their
  tools against a saved docker-archive tarball (`--driver tar`, `--input`), so no
  daemon socket is mounted. Only testcontainers needs the socket, because it
  manages a live container lifecycle. podman's role in these two axes is reduced
  to building and exporting the image.
- **The tools are proto-pinned host binaries, not container images.**
  container-structure-test and Trivy each have a vendored `proto-plugins/*.toml`
  manifest, so their versions are reproducible with no `:latest` image tag or
  arch emulation. Only testcontainers is language-bound (a Rust dev-dep here).

## Doing this for Python

Because only the runtime axis is language-specific, porting this to a Python
service (e.g. a FastAPI app) changes **one of the three tools** and leaves the
rest untouched.

**Runtime — swap the test framework, keep the pattern.** Use `pytest` +
[`testcontainers`](https://pypi.org/project/testcontainers/) (the Python library)

- `httpx`. The container is a context manager, so cleanup on `__exit__` plays the
  same role Rust's `Drop` does:

```python
# tests/test_image.py
import os
import httpx
from testcontainers.core.container import DockerContainer
from testcontainers.core.waiting_utils import wait_for_logs


def test_image_serves_endpoints():
    image = os.environ.get("IMAGE_REF", "myapp:prod")
    with DockerContainer(image).with_exposed_ports(8000) as container:
        wait_for_logs(container, "Application startup complete")
        port = container.get_exposed_port(8000)
        base = f"http://localhost:{port}"
        assert httpx.get(f"{base}/health").status_code == 200
        assert httpx.get(f"{base}/").status_code == 200
    # container stops on __exit__ — same automatic teardown as Rust's Drop
```

This slots into our Python testing standards (uv + pytest + moon) as an
integration test, and honours the same Podman wiring (`DOCKER_HOST`,
`TESTCONTAINERS_RYUK_DISABLED`).

**Structure and hygiene — identical, no change.** `container-structure-test` and
Trivy read the built image, not the source, so the exact same `structure-test.yaml`
shape and the exact same Trivy invocation apply. The only edits are the asserted
values (the binary path, the exposed port, the entrypoint) — not the tools or the
tasks.

**The one real difference is the Dockerfile, not the tests.** A Python app can't
target `scratch`: it ships an interpreter and `site-packages`, so there is no
static self-contained binary to drop into an empty image. The minimal base is one
tier up — typically `gcr.io/distroless/python3` or a `python:*-slim` — which is
larger than our ~3.4 MB Rust image but still lean. The test _strategy_ is
unchanged; the structural assertions simply describe a different filesystem
(e.g. `/app` present, interpreter present, non-root user), and Trivy now has an OS
and Python packages to actually scan (so the hygiene axis does more work, and
matters more).

### The moon shape is the same

The task names, dependencies, and aggregate are identical — only the body of
`image-test` changes (pytest instead of nextest). Each task is defined inline
with moon's `script:` field, so there are no wrapper shell files:

```yaml
tasks:
  image-test: # Python: pytest instead of nextest
    script: 'cd backend && uv run pytest tests/test_image.py'
    deps: ["~:image-build"]
  image-structure: # unchanged — reads the built image, not the source
    script: | # podman save → container-structure-test --driver tar
      tar="$(mktemp).tar"; trap 'rm -f "$tar"' EXIT
      podman save --format docker-archive -o "$tar" myapp:prod
      container-structure-test test --driver tar --image "$tar" --config structure-test.yaml
  image-scan: # unchanged
    script: | # podman save → trivy --input
      tar="$(mktemp).tar"; trap 'rm -f "$tar"' EXIT
      podman save --format docker-archive -o "$tar" myapp:prod
      podman run --rm -v "$tar:/work/image.tar:ro" aquasec/trivy:latest \
        image --input /work/image.tar --severity HIGH,CRITICAL --exit-code 1
  image-check:
    deps: ["~:image-test", "~:image-structure", "~:image-scan"]
```

## Summary

- Test the **image**, not a proxy for it — it is the artifact you deploy.
- Three axes: **runtime** (behaviour), **structure** (assembly), **hygiene**
  (safety). Cover all three; a single smoke script covers only the first.
- **Runtime is the only language-specific axis.** Structure and hygiene operate
  on the built image, so they port for free.
- **Porting to Python** means: pytest + testcontainers for runtime, the same
  container-structure-test + Trivy for the other two, and a different (non-`scratch`)
  base image in the Dockerfile — with the moon task graph unchanged.
