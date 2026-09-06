# Haskell in the wild — what it's for, and the bugs it deletes

A companion to [`genisis.md`](./genisis.md). That document argues why Haskell
suits *this* pilot. This one zooms out: where Haskell actually earns its keep in
production, and — the part worth internalising — *which entire classes of
runtime error it makes impossible*, not merely less likely.

The through-line: Haskell trades **upfront ceremony** (types, totality, effects
in the signature) for the **disappearance of whole bug categories**. You pay the
compiler now so the pager stays quiet later. That trade only pays off for
certain shapes of problem — so this doc also says plainly where it does *not*.

---

## Part 1 — Where it's used, and why there

Haskell is a niche language, but its niches are revealing: they are almost all
places where **a wrong answer is expensive** and **the logic is intricate**.

| Domain | Who (real examples) | Why Haskell |
| --- | --- | --- |
| **Anti-abuse at scale** | Meta's *Sigma* / *Haxl* — spam & malware filtering on every post and message | A pure rules engine that's safe to run untrusted-ish logic on, with automatic concurrent data-fetching (Haxl). Correctness + safe parallelism. |
| **Finance / trading** | Standard Chartered (one of the largest Haskell codebases, a strict dialect "Mu"), Barclays (exotic-derivative DSLs), Tsuru Capital | A mispriced trade is a real loss. Typed DSLs model payoffs; purity makes the maths auditable. |
| **Blockchain** | Cardano / IOHK (the whole stack; *Plutus* smart contracts) | Consensus and on-chain money are unforgiving. Pairs naturally with formal methods. |
| **High-assurance / crypto** | Galois (*Cryptol*, *SAW* — US-government high-assurance crypto) | The domain *is* proof. Haskell is the host for the proof tooling. |
| **Compilers & language tools** | GHC itself, the *Elm* and *PureScript* compilers, *Agda*, *Idris*, *Dhall* (typed config) | ASTs are algebraic data types; transformations are pattern matches. This is Haskell's home turf. |
| **Developer tools / linters** | *ShellCheck* (shell linter), *hadolint* (Dockerfile linter), *Pandoc* (universal document converter) | Parse → analyse → transform. Total functions over a rich AST; correctness users can trust. |
| **Web backends / APIs** | *Hasura* (instant GraphQL), Mercury (banking), Co‑Star (astrology app backend) | Type-safe request → DB → response pipelines; refactors that don't break silently. |

Notice the pattern: **parsers, compilers, financial logic, consensus,
proofs.** All are *symbol-pushing over a structured domain with high cost of
error* — the exact shape the language flatters (and the shape `kairos` is a
miniature of).

---

## Part 2 — The bug classes it structurally eliminates

This is the heart of it. For each, the bug as you'd hit it in Python/JS/Go, and
*why the Haskell version cannot compile*.

### 1. Null dereference — gone

> The billion-dollar mistake. `user.address.city` when `address` is null.

Haskell has **no null**. Absence is the value `Nothing` of type `Maybe a`, and
the type forces you to handle the `Nothing` case before you can touch the `a`.
There is no syntax for "use it and hope". You saw this in `Kairos.Interval`:
`interval` returns `Maybe (Interval a)`, so a caller *cannot* forget that
construction can fail.

### 2. Unhandled case — caught at compile time

> A new `enum` variant ships, and three `switch` statements silently fall
> through the wrong branch.

A `data` type lists every case; pattern matching over it is checked for
**exhaustiveness** (`-Wall`). Add a 14th constructor to a 13-case type and
*every* match that doesn't handle it becomes a compile error pointing at the
exact line. The compiler maintains the invariant for you.

### 3. Implicit side effects — impossible

> A function called "for its return value" also writes a file, mutates a
> global, or fires a network call. Refactoring becomes terrifying.

Effects live **in the type**. `a -> b` provably performs no I/O, no mutation, no
exception-throwing — it is a function in the mathematical sense. Only `a -> IO b`
can touch the world. This is *referential transparency*: equals can be
substituted for equals, so refactoring is fearless and reasoning is local.

### 4. Data races / torn state — designed out

> Two threads mutate the same field; you get a value that never existed.

Data is **immutable by default**, so most shared-state races simply can't arise.
For genuine shared mutable state, `STM` (software transactional memory) gives you
composable, lock-free transactions that retry instead of corrupting. (Meta's
Haxl leans on exactly this to fetch thousands of things concurrently and safely.)

### 5. Type confusion / unit errors — newtypes

> A `userId :: Int` gets passed where an `orderId :: Int` was expected. Or
> metres where the API wanted feet (the $125M Mars Climate Orbiter bug).

A `newtype UserId = UserId Int` is a **distinct type at zero runtime cost**. Mix
`UserId` and `OrderId` and it doesn't compile, even though both are "just an
Int". Whole categories of mix-up vanish.

### 6. Silently-ignored errors — forced threading

> A function returns an error code nobody checks; execution sails on with bad
> state.

`Either e a` makes the failure part of the value. You cannot get at the `a`
without confronting the `e` — there is no equivalent of ignoring a return value
or letting an unchecked exception slip past. Failure is in the signature, so the
caller is on the hook for it.

### 7. Illegal states — unrepresentable

> An object with `status = "shipped"` but a null `shippedAt`. Defensive `if`s
> sprinkled everywhere to re-check invariants the type should have guaranteed.

You **make illegal states unrepresentable**: design the type so the bad
combination has no value. Hide the constructor behind a smart constructor (our
`Interval` again — a backwards interval has no representation), or model states
as an ADT where each carries exactly the data that state needs. The defensive
re-checks disappear because the situation they guard against can't occur.

---

## Part 3 — The sweet-spot shape

Add the above up and a profile emerges. Haskell pays off hardest when the
problem is:

- **logic-dense** — the value is in the transformation, not in I/O plumbing;
- **high cost of error** — money, consensus, security, a compiler others depend on;
- **long-lived and refactored often** — types are what make large changes safe;
- **modellable as data + transformations** — ASTs, protocols, state machines, algebras.

That is why the wild-caught examples cluster on compilers, finance, blockchain,
and parsers. It is also a fair description of `kairos`: a small, total, law-rich
algebra with almost no I/O.

---

## Part 4 — Where it's the *wrong* tool (the honest counterweight)

Eliminating bug classes is not free, and Haskell is a poor fit when:

- **You need predictable latency.** Lazy evaluation and GC make hard-real-time
  and tight tail-latency work hard. (Strict annotations help, but it's swimming
  upstream.)
- **The work is mostly I/O glue or throwaway scripting.** The type ceremony buys
  little when there's no intricate logic to protect, and iteration speed matters
  more than guarantees. A shell script or Python is often the right answer.
- **You're memory- or embedded-constrained.** The runtime and laziness carry
  overhead.
- **The team has no FP background.** The learning curve is real and the hiring
  pool is small — a genuine organisational cost, not just a personal one.

And Haskell has **one notable bug class of its own**: the **space leak**. Lazy
evaluation can quietly pile up unevaluated thunks until memory balloons — a
failure mode strict languages don't have. It's the price of laziness, and
diagnosing it is a real Haskell skill.

> The summary: Haskell deletes the bugs that come from *forgetting* — forgetting
> a null check, a case, an error, an effect, an invariant. It does not delete
> the bugs that come from *getting the logic itself wrong*, and it introduces
> laziness's own footgun. Reach for it where forgetting is expensive and the
> logic is worth protecting. Reach elsewhere where it isn't.
