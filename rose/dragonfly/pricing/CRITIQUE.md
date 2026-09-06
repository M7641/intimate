# Critique — is header-routed WASM pricing a strange direction?

Short answer: **the mechanism is sound and even elegant, but this particular
example undersells when you'd actually reach for it, and the header is the
wrong thing to route on in production.** It is not strange — it sits on a
recognised spectrum — but it's worth being precise about what it buys you.

## What it gets right

- **Logic isolation without process isolation.** The classic alternatives are
  "one instance per department" (heavy, but isolated) or "one instance with
  `if department == 'retail'` branches in the source" (the bespoke-branch smell
  `genisis.md` is trying to escape). WASM-per-department is a genuine third
  path: one process, but each department's rules are a separately-compiled,
  separately-versionable, sandboxed unit. A bug in wholesale's arithmetic
  cannot corrupt retail's — and the fallback demonstrates that.
- **Independent release cadence.** A department can ship a new `.wasm` without
  the host rebuilding or redeploying its _logic_. That's the real prize, and
  it's why this beats a config flag once the logic stops being expressible as
  data.
- **Honest failure mode.** Falling back to `default` on a plugin error is the
  right instinct for a pricing path — wrong-but-safe beats down.

## Where it's weak

### 1. The _demo's example_ is too simple — but only if the logic is first-party

Retail and wholesale here differ only in **numbers** (markup %, discount
tiers, thresholds). If _you_ author both, that's data, not code:

> Premature plugin systems are over-engineering. It makes sense when you've
> exhausted what configuration can express.

Everything in these two demo plugins could be a row in a `pricing_policy`
table: `{markup_pct, volume_tiers[], segment_bonuses[]}` — simpler,
hot-reloadable, editable by a non-engineer.

**This objection dissolves under the actual constraint.** The real
requirement is: _a fixed core that must safely run code supplied by other
providers._ Config is then off the table for two reasons — you cannot express
an arbitrary third party's logic as table rows, and config does not need a
sandbox because it is not executable code. Running untrusted third-party
**code** safely is precisely the 20% `genisis.md` says justifies a plugin
system. So critique #1 only ever applied to a first-party scenario; it does
not apply here. (The demo's _example_ is still a poor advertisement — it
should show logic no config schema could express — but the _direction_ is
sound. See "If not WASM, then what?" below.)

### 2. Routing on a client-set header conflates transport with identity

`X-Department: wholesale` is **caller-controlled and trivially spoofable**.
For a demo that's fine; in production "which pricing applies" is a _domain_
decision that should derive from **authenticated context** (the tenant/user in
the auth token, an org's configured department), resolved in middleware — then
used to select the engine. Letting any client pick its own pricing engine by
flipping a header is, by itself, the genuinely questionable part of the design.
The header should stand in for "resolved department context," not _be_ it.

### 3. "Collapse two instances into one" trades isolation for density

The premise was two instances → one. Be clear about what's lost:

|                                       | Two instances               | One host + plugin routing          |
| ------------------------------------- | --------------------------- | ---------------------------------- |
| Logic isolation                       | Strong (separate processes) | Medium (WASM sandbox, shared host) |
| Blast radius of a host bug            | One department              | **Both departments**               |
| Noisy-neighbour / resource contention | None                        | Shared CPU/memory                  |
| Independent scaling                   | Yes                         | No (scale the whole host)          |
| Infra & deploy cost                   | Higher                      | Lower                              |
| Per-department logic versioning       | Per deploy                  | **Per plugin (better)**            |

If the departments have very different load profiles or strict isolation/
compliance needs, two instances may still be right. The plugin model wins when
you have _many_ providers (the cost of N instances dominates) and the
isolation a sandbox gives is enough.

## If not WASM, then what?

Given the hard constraint — _a fixed core that must safely run code from other
providers_ — some form of sandbox is unavoidable. "Use config instead" is not
an option: you cannot express an untrusted third party's arbitrary logic as
data. So the real question is _which_ isolation technology, judged on two axes
that matter most here: **boundary strength** (how big a Trusted Computing Base
you must assume is correct) and **per-invocation latency/density** (pricing is
a hot path — it may run per request or per row).

| Mechanism                                                         | Boundary strength                                            | Latency / density                          | Languages            | In-process?      | Fit for this case                                           |
| ----------------------------------------------------------------- | ------------------------------------------------------------ | ------------------------------------------ | -------------------- | ---------------- | ----------------------------------------------------------- |
| Language sandbox (RestrictedPython, Lua, ex-Java SecurityManager) | Weak — leaks, often deprecated                               | Excellent                                  | one                  | yes              | Rejected by `genisis.md`, rightly                           |
| OS container (Docker/OCI)                                         | Medium — **shared kernel**, escapes exist                    | Heavy cold start, big per-tenant footprint | any                  | no (IPC/network) | Re-introduces "webhook" latency on the hot path             |
| gVisor (userspace kernel)                                         | Medium-strong                                                | Syscall overhead                           | any                  | no               | Better than OCI, still out-of-process                       |
| microVM (Firecracker)                                             | Strong (real VM boundary)                                    | ~125 ms boot                               | any                  | no               | Great for long-lived per-tenant workers, not a per-call hop |
| V8 isolate (Cloudflare Workers model)                             | Medium-strong (rests on V8)                                  | Excellent (~ms)                            | **JS only**          | yes              | Ideal — _if_ you accept the JavaScript lock-in              |
| **WASM** (Wasmtime/Extism)                                        | Strong — bytecode-level memory isolation, capabilities, fuel | Low (ms instantiate, fast call)            | many (Rust/Go/C/AS…) | yes              | **The only option that satisfies all three at once**        |

The reasoning lands on WASM _non-accidentally_:

1. Untrusted third-party code → a real sandbox is mandatory (rules out
   language sandboxes and running native code directly).
2. Called on the pricing hot path → must be **in-process / low-latency**
   (rules out containers, gVisor, microVMs — they force an IPC/network hop,
   the exact webhook latency `genisis.md` rejects).
3. Code arrives from _varied_ providers → must be **polyglot** (rules out the
   JS-locked V8 isolate).

What survives the intersection is WASM. Two nuances worth keeping:

- **It is rarely WASM _vs_ containers — usually both.** Run the WASM runtime
  itself inside a container or microVM for defence in depth (Shopify
  Functions, Fastly Compute, Fermyon do variants). WASM is the _fine, fast,
  per-provider_ isolation layer; the container stays the _coarse deployment_
  layer.
- **WASM's real weakness is maturity/DX, not security** — the Component Model
  isn't stable, debugging is harder, and the host-call boundary leaks when
  passing complex types. Those are engineering costs, not holes.

## Verdict

Not strange — and under the stated constraint, **well chosen**. It's a
legitimate pattern (the shape of feature-flagged logic, serverless function
routers, Shopify Functions), and for "fixed core + untrusted third-party code
on a hot path, from many languages" WASM is close to the only tool that fits.
Critique #1 ("use config") applies only to first-party logic and does **not**
apply here. What remains true regardless of technology:

1. **Route on authenticated provider/department context, not a raw header.**
   (Keep the header only as the demo's stand-in.)
2. **Be explicit about the isolation trade** vs the two-instance baseline, and
   layer WASM inside a coarser boundary (container/microVM) for defence in
   depth rather than treating the runtime as the only wall.
3. **Make the demo's example harder** — logic no config schema could express —
   so the artifact argues _for_ the direction the constraint already justifies.

The plumbing is worth keeping; the constraint justifies the mechanism. The
only thing that should "get harder" is the example, not the architecture.
