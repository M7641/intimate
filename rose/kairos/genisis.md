# genisis — temporal reasoning, and why Haskell suits it

A companion to the [`README.md`](./README.md). The README explains *what*
`kairos` does and *how* to run it. This document answers one question: why is a
mathematical domain like temporal reasoning a good first thing to write in
Haskell — and what does the language actually buy you here?

---

## Part 0 — The domain has laws

Two time intervals can sit in only so many configurations: one before the
other, touching end-to-start, overlapping, nested, sharing an edge, identical.
James Allen catalogued them in 1983: **13 relations**, and any two proper
intervals stand in *exactly one* of them. That "exactly one" is not a wish — it
is a theorem. The relations also pair up: each has a mirror (`before` ↔ `after`),
and mirroring twice returns you home.

When a domain comes with theorems already attached, you want a language that
lets you write the theorems *down, next to the code, and test them*. That is the
whole pitch for Haskell here.

---

## Part 1 — Three things the language does for you

### 1. Make illegal states unrepresentable

`Interval` hides its constructor and only lets you in through
`interval :: Ord a => a -> a -> Maybe (Interval a)`, which returns `Nothing`
unless `start < end`. So *nowhere downstream* can a backwards interval exist —
not because everyone remembered to check, but because there is no syntax for it.
The invariant moved out of runtime discipline and into the type.

This is the Haskell habit worth taking away: **push the constraint into the
type, then stop checking it**. `relate` never asks "is this interval valid?" —
it cannot receive an invalid one.

### 2. Total functions, checked for exhaustiveness

`relate` is a single chain of guards over four endpoint comparisons. Because the
intervals are proper, those four comparisons fully determine the relation —
`relate` always returns, and returns exactly one answer. The compiler's `-Wall`
flag warns if a case is unreachable or missing. The "exactly one of 13" theorem
becomes something the build enforces, not a comment you hope stays true.

### 3. Laws as tests — QuickCheck

Haskell is where property-based testing was invented. Instead of a handful of
hand-picked examples, you state a *law* and the machine generates hundreds of
random inputs trying to falsify it. `test/Spec.hs` states three:

- `converse` is its own inverse (mirror twice → identity);
- an interval is `Equal` only to itself;
- `relate x y` is always the mirror of `relate y x`.

Run `cabal test` and QuickCheck *tries to break them*. Passing is evidence the
algebra is internally consistent — the kind of evidence a dynamically-typed
prototype never gives you for free.

---

## Part 2 — Why this connects to `saga`

[`saga`](../saga/genisis.md) is a bi-temporal context store: every fact carries
**valid time** (when it was true in the world) and **transaction time** (when
the system believed it). Those are two intervals — so the heavy-sounding phrase
"bi-temporal point query" collapses into two interval-membership tests
(`Kairos.Bitemporal.asOf`). The expressiveness is in the *model* (two
timelines), not in machinery. `kairos` is `saga`'s temporal core with everything
else stripped away, so you can see the algebra without the storage, embeddings,
and retrieval around it.

---

## Part 3 — Where it would go next

The pilot stops at `relate` and `converse`. The next rung of the algebra is
**composition**: if `x` is `before` `y` and `y` `overlaps` `z`, what can `x`–`z`
be? The answer is a *set* of relations, and composing constraints across many
intervals is how you do real scheduling and temporal-constraint solving (the
classic 13×13 composition table, then path-consistency over it). That is a
natural — and very Haskell — second chapter: a finite relation lattice with its
own laws to QuickCheck.

> Not every prototype belongs in Haskell. The fit here is specific: the domain
> is small, total, and law-rich, with no I/O to speak of. That is exactly the
> shape the language flatters — and exactly why it is a good first one.
