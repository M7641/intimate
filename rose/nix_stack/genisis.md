# Nix — Reproducible Environments

The Principles note says "total reproducibility." Mise handles tool versions. Docker handles deployment containers. But neither guarantees that the environment on your machine, your colleague's machine, and CI are _identical_. Nix does.

## What Nix Actually Is

Nix is a package manager where every package is built in isolation and addressed by a hash of all its inputs (source code, dependencies, build flags, compiler version). If any input changes, the hash changes, so you get a different package. Two machines with the same Nix expression are guaranteed to produce identical environments. No "works on my machine." No version drift. No implicit system dependencies.

The key insight: Nix treats environment configuration as a _function_ from inputs to outputs. Same inputs, same outputs. This is the parsing-not-validation philosophy applied to infrastructure — the environment is a type, and illegal configurations don't build.

## Where It Fits

**Development shells (nix develop / devbox)**
Replace `mise` + manual setup with a single `flake.nix` that declares: Rust toolchain (exact version), Python with specific packages, PostgreSQL client libraries, protobuf compiler, any system dependencies. Run `nix develop` and you're in a shell with exactly those tools. No contamination from system packages. No "I have Python 3.11 but you have 3.12."

Devbox (by Jetpack) is the gentler on-ramp — it wraps Nix with a simpler interface (`devbox.json` instead of Nix expressions). Worth starting here.

**CI pipelines**
The same `flake.nix` that defines the dev shell defines the CI environment. No more maintaining separate Dockerfiles for CI that drift from development. `nix build` in CI uses the same derivations as `nix develop` locally.

**Rust + Python polyglot projects**
This is where Nix shines for the current stack. Rust projects with Python bindings (PyO3) need both toolchains pinned and compatible. Nix can declare the exact Rust nightly, the exact Python version, maturin for building wheels, and all C libraries — in one expression. The ML System Architecture note describes a shared Rust feature engine bridged to Python — Nix makes that bridge reproducible.

**Data pipeline environments**
For DataMove: pin the exact versions of dbt, SQLFluff, the Snowflake connector, the PostgreSQL driver. Every developer and every CI run uses identical tooling. Schema migrations behave the same everywhere.

## What Nix Is Not Good At

Runtime containers in production — Docker is still better here because Kubernetes expects OCI images. But Nix can _build_ Docker images (with `dockerTools.buildImage`), producing minimal, reproducible images without a Dockerfile. The image contains exactly what Nix declares, nothing more.

Nix also has a steep learning curve. The Nix language is functional and unfamiliar. Flakes (the modern Nix interface) improve this but are still more complex than a `Dockerfile`. The trade-off is: higher upfront cost, lower ongoing cost from never debugging environment drift again.

## Practical Starting Point

1. Install Nix with flakes enabled
2. Add a `flake.nix` to one project (start with a Rust project — Nix has excellent Rust support via crane or naersk)
3. Define a dev shell with the exact toolchain
4. Verify that `nix develop` on a clean machine produces a working environment
5. If that feels valuable, extend to CI and then to the Python/Rust polyglot projects

The goal isn't to Nix everything overnight. It's to make "total reproducibility" a property of the build system, not a discipline you maintain manually.
