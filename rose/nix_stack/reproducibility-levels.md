# Reproducibility — which layer are we actually pinning?

A note from the `nix_stack` pilot, comparing the tools by the _level_ at which
they guarantee reproducibility. The headline: **mise and Nix are not competitors
on the same axis — they reproduce different layers of the stack.**

## The stack, bottom to top

```
┌─────────────────────────────────────────────┐
│  4. Application code + deps  (Cargo.lock,    │  ← language lockfiles
│                               uv.lock)       │
├─────────────────────────────────────────────┤
│  3. Tool / runtime versions  (rustc, python, │  ← mise pins HERE
│                               uv, node, bun) │
├─────────────────────────────────────────────┤
│  2. System / C libraries     (openssl, libpq,│  ← Nix pins HERE too
│                               protobuf, glibc)│
├─────────────────────────────────────────────┤
│  1. OS / kernel              (the base image,│  ← NixOS / Nix images
│                               system layout) │     pin HERE too
└─────────────────────────────────────────────┘
```

Everyone agrees on layer 4 — that is what `Cargo.lock` and `uv.lock` are for, and
both tools defer to them. The difference is how far _down_ the stack each tool's
guarantee reaches.

## What mise reproduces: the tools (layer 3)

`mise` pins **tool and runtime versions** and pulls them straight from each
tool's own upstream releases, locked in `mise.lock`. Every developer and CI run
gets the same `rustc`, `python`, `uv`, `node`, etc.

- **Strength:** because it tracks upstream directly, it can be on the _cutting
  edge_ — `uv@latest` or `python@3.14` are available the day they ship, not
  whenever a curated package set catches up. Single binary, no daemon, trivial
  setup. This is why the rest of this monorepo runs on mise.
- **Boundary:** mise does **not** control layers 2 and 1. The tools it installs
  still dynamically link against whatever system libraries (openssl, libpq,
  glibc, …) happen to be on the host. Two machines with identical `mise.lock`
  can still differ if their underlying OS libraries differ. For pure
  Rust/Python/TS/Node work this almost never bites; it only matters when a
  package depends on a specific native system library being reproducible.

In one line: **mise gives you a reproducible _toolchain_.**

## What Nix reproduces: the whole environment (layers 1–3)

`nix` addresses every package — _including_ system and C libraries, and
optionally the OS layout itself — by a hash of all its inputs. Same inputs →
byte-identical output, with no dependency on what the host already has installed.
It can go all the way down: `dockerTools.buildImage` produces a minimal,
reproducible OS image without a Dockerfile, and NixOS reproduces the entire
operating system from one expression.

- **Strength:** _total_ reproducibility. The system libraries a binary links
  against are pinned just like the binary itself. This is the only tool here that
  closes layer 2 and layer 1.
- **Cost:** it pays for that hermetic guarantee with a curated package set
  (nixpkgs), which inherently lags upstream — the opposite of cutting edge — plus
  a steeper install and a functional language to author. The `nix develop` pain
  documented in this pilot's README is the price of admission for layer-2/1
  guarantees you may not need day to day.

In one line: **Nix gives you a reproducible _operating environment_.**

## The decision for this repo

For **everyday development** — dropping in and out of many packages, on the
cutting edge — tool-level reproducibility is the right altitude, and `mise`
already provides it across the monorepo. Pinning OS-level details would be
over-solving the problem and would cost us the freshness we want.

**Reach for Nix only if we ever need reproducibility to descend below layer 3** —
that is, when pinning tool _versions_ is provably not enough and we need the
_system libraries or the OS itself_ to be identical everywhere. Realistic
triggers:

- A package with gnarly native/C dependencies that "works on my machine" because
  of a host library, and we need it identical across the team and CI.
- A production artifact that must be bit-for-bit reproducible for security,
  compliance, or audit reasons — built once, verifiable forever.
- Reproducing the OS layer itself (e.g. a NixOS host or a hermetic base image),
  where "the same tools" is not the same as "the same machine".

Until one of those is real, mise is the better fit on every axis we actually care
about (setup, DX, cutting edge), and Nix stays on the shelf as the escape hatch
for _total_, OS-level reproducibility — used surgically on the one package or the
one prod build that needs it, not imposed on the whole dev loop.
