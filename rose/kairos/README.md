# kairos — Allen's interval algebra in Haskell

A small, pure pilot in temporal reasoning. It models the **13 Allen interval
relations** between two time intervals, checks the algebra's laws with
QuickCheck, and layers a **bi-temporal fact store** on top (the same problem
[`saga`](../saga) tackles, reduced to its mathematical core).

The companion [`genisis.md`](./genisis.md) explains *why* — what the algebra is
and what Haskell brings to it. [`in-the-wild.md`](./in-the-wild.md) zooms out:
where Haskell is used in production and the bug classes it deletes by
construction. [`vs-rust.md`](./vs-rust.md) compares the two in the niches they
share, and when to reach for which. This file is *what* and *how*.

## Layout

```
src/Kairos/Interval.hs    a proper interval; the constructor is hidden so a
                          malformed interval cannot exist
src/Kairos/Allen.hs       the 13 relations + `relate` (total) + `converse`
src/Kairos/Bitemporal.hs  two timelines (valid + transaction), one query: `asOf`
app/Main.hs               a runnable tour
test/Spec.hs              the algebra's laws, as QuickCheck properties
```

## Setup

**1. Install the Haskell toolchain** (GHC the compiler + Cabal the build tool).
One command — accept the defaults, then open a new terminal so `cabal` is on
your `PATH`:

```sh
curl --proto '=https' --tlsv1.2 -sSf https://get-ghcup.haskell.org | sh
```

**2. Build and run**, from this directory:

```sh
cabal run kairos     # the demo (Allen relations + a bi-temporal query)
cabal test           # QuickCheck the laws (hundreds of random interval pairs)
```

That's it — the first `cabal` command also fetches QuickCheck and compiles.

> Not pinned via proto, unlike the rest of the repo: `ghcup` is to Haskell what
> `rustup` is to Rust. proto handles single-binary, 3-part-semver tools; the
> Haskell toolchain uses 4-component versions and a build step, so ghcup owns it.

## What to read first

Start in `src/Kairos/Allen.hs` — `relate` is the whole idea on one screen:
four endpoint comparisons decide which of 13 relations holds. Then `test/Spec.hs`
to see how a *law* ("relating x to y mirrors relating y to x") becomes a test the
machine tries to break.
