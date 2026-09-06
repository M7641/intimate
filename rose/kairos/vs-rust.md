# Haskell vs Rust — two ways to make illegal states unrepresentable

A companion to [`in-the-wild.md`](./in-the-wild.md). That doc maps where Haskell
is used and the bug classes it deletes. This one answers the natural follow-up
for a Rust shop: **in the niches the two languages share, which wins, and why?**

Both are the great "if it compiles, it works (mostly)" languages. They are
cousins, not strangers — so the comparison is close, and the dividing lines are
specific rather than vibes.

---

## The one-sentence model

> **Haskell maximises reasoning power and abstraction, at the cost of runtime
> control. Rust maximises runtime control, at the cost of abstraction power.**

They share a type-system philosophy and diverge on a single fault line:
**GC + laziness + enforced purity** (Haskell) versus **ownership + strict
evaluation + no runtime** (Rust).

---

## Shared DNA — why Rust ate territory Haskell pioneered

These are not opposites; they overlap heavily. Both have:

- **ADTs + exhaustive pattern matching** — Rust's `enum` _is_ Haskell's `data`;
- **no null** — `Option` / `Maybe`;
- **`Result` / `Either`** for errors as values;
- **traits ≈ typeclasses**;
- immutability by default and HM-derived type inference.

So most of the _"bugs of forgetting"_ from [`in-the-wild.md`](./in-the-wild.md)
— null derefs, unhandled cases, ignored errors, illegal states — **Rust
eliminates too**. That shared core is exactly why Rust has displaced Haskell in
a lot of places (dev tools above all): you keep ~80% of the guarantees and gain
a fast static binary with no runtime.

---

## The fault line

| Axis                   | Haskell                                                       | Rust                                                             |
| ---------------------- | ------------------------------------------------------------- | ---------------------------------------------------------------- |
| **Memory / runtime**   | GC + lazy thunks; space leaks possible; GC pauses             | No GC, RAII, deterministic, near-C                               |
| **Evaluation**         | Lazy (elegant; also a memory footgun)                         | Strict                                                           |
| **Effects**            | _Enforced in the type_ (`IO`) — purity guaranteed             | No enforced purity; the discipline is about aliasing (ownership) |
| **Abstraction**        | HKT, monads, GADTs, type families, dependent-types-adjacent   | Powerful traits but **no HKT**; macros to compensate             |
| **Concurrency**        | Green threads (millions), STM; no compile-time race guarantee | `Send`/`Sync`: **data-race freedom proven at compile time**      |
| **Ecosystem / hiring** | Niche, stable, smaller; cabal/stack                           | Large, growing, `cargo`, deep hiring pool                        |

The lever is the thing to remember: **purity** is Haskell's, **ownership** is
Rust's. One buys referential transparency; the other buys zero-cost and
predictable latency.

---

## Niche by niche

The areas where both actually compete (the niches from `in-the-wild.md`):

| Shared niche                   | Leans Haskell                              | Leans Rust                              | What decides it                                                                                                                            |
| ------------------------------ | ------------------------------------------ | --------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------ |
| **Compilers / language tools** | GHC, Elm, PureScript, Agda, Dhall          | rustc, SWC, **Ruff**, oxc, Biome        | Elegance of implementing intricate semantics (Haskell) **vs** the tool's _runtime speed is a feature_ because it runs on every save (Rust) |
| **Finance / trading**          | Standard Chartered, Barclays (payoff DSLs) | microsecond-latency HFT                 | The _modelling / correctness_ layer (Haskell) **vs** the _latency-critical execution_ layer where GC is disqualifying (Rust)               |
| **Blockchain**                 | Cardano / IOHK, Plutus                     | Solana, Polkadot, Near                  | A _formal-methods / correctness_ culture (Haskell) **vs** _validator performance_ + ecosystem volume (Rust has won the volume)             |
| **High assurance / proofs**    | Galois (Cryptol, SAW), Agda / Idris        | the _verified_ target (hax, Aeneas)     | Being the _metalanguage of proofs_ (Haskell) **vs** being the fast _verified artefact_ (Rust)                                              |
| **Parsers / transformation**   | megaparsec, attoparsec (peak ergonomics)   | nom, chumsky, pest                      | Expressiveness (Haskell) **vs** raw throughput (Rust)                                                                                      |
| **Deep typed DSLs**            | HKT + monads + GADTs                       | traits + macros, but a ceiling (no HKT) | Haskell wins once the DSL gets deep                                                                                                        |

---

## The decision rule

In a _shared_ niche, one question usually settles it:

- **Does the system's performance / latency matter to its users?** (runs
  constantly, low latency, embedded, no GC pauses tolerable) → **Rust**.
- **Is the value in modelling intricate semantics correctly and abstractly,**
  with light I/O and uncritical latency? (a compiler's correctness, a pricing
  DSL, a proof host) → **Haskell**.

---

## What each lacks from the other

- **Rust lacks** higher-kinded types, enforced purity, laziness, do-notation /
  monadic abstraction, dependent types, and GHC's decades of type-system
  research (type families, etc.). Its parser combinators are good, not as clean.
- **Haskell lacks** ownership / no-GC / predictable latency, `Send`/`Sync`
  compile-time data-race freedom, `cargo`'s tooling momentum, and easy small
  static-binary deployment.

And each carries a _second wall_ beyond the shared type-system learning curve:

- **Rust's tax** is the **borrow checker** — fighting lifetimes and aliasing.
- **Haskell's tax** is **reasoning about laziness** (space leaks) and
  monad-transformer stacks that get hairy.

---

## This repo already settled it

The verdict is playing out in this monorepo, no abstraction required:

- [`cocoon`](../../cocoon) — production services, ingest, DB, performance-sensitive
  — is **Rust**.
- The always-on tooling (`ruff`, `sqruff`, `cargo-nextest`, `uv`) is **Rust**,
  chosen for speed.
- [`kairos`](.) — a pure, law-rich algebra with no I/O, an exploration pilot —
  is the shape that leans **Haskell**.

> The pragmatic takeaway for a Rust shop: Rust gives you most of Haskell's
> "can't-compile-the-bug" benefits _plus_ a fast runtime-free binary, so it's
> often the right call **even in niches Haskell invented**. Haskell's residual
> edge is the top slice of abstraction — HKT, purity-by-default, laziness,
> proofs — which only pays for compilers, deep DSLs, and formal methods. Reach
> for Haskell when that slice is the whole point; reach for Rust otherwise.
