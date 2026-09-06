# Gleam and the BEAM — Fault Tolerance with Types

## The Gap Between Rust and Python

Rust gives you performance and safety but makes concurrency verbose. Tokio handles async I/O beautifully, but managing hundreds of independent tasks with supervision, graceful degradation, and hot code reloading requires significant custom infrastructure. The "Postgres Panic" note — where a runtime shutdown conflicted with blocking I/O — illustrates the friction: Rust's ownership model and async runtime interact in ways that require careful choreography.

Python gives you speed of development but its concurrency model is fundamentally limited. The GIL prevents true parallelism. asyncio handles I/O concurrency but not CPU concurrency. Multiprocessing works but shares nothing, making coordination expensive.

The BEAM VM (Erlang's runtime) was built specifically for the problem neither language solves. Millions of lightweight processes, each with isolated memory, communicating through message passing, supervised by a hierarchy that restarts failed processes automatically. A process that crashes doesn't take down its neighbours. The system self-heals.

## Why Gleam, Not Elixir or Erlang

Erlang's syntax is hostile to most developers. Elixir improved the syntax dramatically and built a vibrant ecosystem (Phoenix, LiveView, Ecto). But Elixir is dynamically typed — it has the same category of runtime error problems that the parsing-not-validation philosophy rejects.

Gleam is a statically typed language on the BEAM. It has:

**Algebraic data types and exhaustive pattern matching.** Like Rust's enums, every case must be handled. The compiler catches missing branches. No runtime "function clause error" surprises.

**No exceptions, no nil.** Results are `Result(value, error)` — the same pattern as Rust's `Result<T, E>`. Errors are values, not control flow. You handle them explicitly or propagate them.

**Immutable data.** Everything is immutable by default. No shared mutable state. The BEAM's message-passing model enforces this at the runtime level; Gleam enforces it at the language level.

**Interop with Erlang and Elixir.** Gleam compiles to BEAM bytecode and can call any Erlang or Elixir library. The entire OTP ecosystem (supervision trees, gen_servers, ETS tables, distributed Erlang) is available. Phoenix web framework, Ecto database library, Broadway data pipeline library — all usable from Gleam.

**Also compiles to JavaScript.** Gleam's secondary target is JavaScript, meaning the same types and logic can run in the browser or in Node. Not the primary use case, but interesting for shared validation logic between server and client.

## The Supervision Model

This is the BEAM's killer feature and the thing most relevant to the current architecture.

A supervision tree is a hierarchy of processes where parent processes monitor children. If a child crashes, the supervisor restarts it according to a strategy:

- **one_for_one:** Only the crashed child is restarted. Other children are unaffected.
- **one_for_all:** All children are restarted. Used when children are interdependent.
- **rest_for_one:** The crashed child and all children started after it are restarted.

For a multi-tenant data pipeline where each customer has an independent pipeline process, one_for_one supervision means: Speedy's pipeline crashes, Speedy's pipeline restarts, every other customer is unaffected. No mutex, no shared state, no coordinated shutdown. The isolation is structural.

Compare this to the current architecture where customer pipelines likely run as separate tasks within a shared Tokio runtime. A panic in one task can (depending on implementation) propagate to the runtime. The "Postgres Panic" note shows exactly this: a cleanup failure during shutdown affects the entire process. On the BEAM, that failure affects one process. The supervisor restarts it. Everything else continues.

## Where It Fits

**Pipeline orchestration.** Not the individual data transformations (keep those in Rust for performance) but the orchestration layer that manages customer pipelines, handles failures, coordinates retries, and routes messages. The BEAM's process model is purpose-built for this.

**Real-time event processing.** If NATS delivers events that need to be routed to customer-specific handlers, each handler can be a BEAM process. Broadway (an Elixir/Gleam library) provides back-pressure, batching, and fault-tolerant event processing out of the box.

**The agentic coordination layer.** Agentic workflows involve multiple concurrent activities that may fail independently. The BEAM's supervision model handles this more naturally than manual state management in Rust or Python. Each agent step is a process. The workflow is a supervision tree. Failure is expected and handled structurally.

**Long-lived connections.** WebSocket connections, SSE streams, gRPC streaming — the BEAM handles millions of concurrent connections with processes so cheap (2KB each) that one-process-per-connection is the default pattern, not an optimisation.

## The Three-Language Stack

Gleam doesn't replace Rust or Python. It occupies the orchestration layer between them:

```
Python — ML training, data science, rapid prototyping
Gleam  — orchestration, supervision, concurrent coordination, real-time events
Rust   — latency-critical serving, feature computation, data transformation
```

This is a more honest language split than Rust + Python alone. Rust is excellent at fast, correct computation. Python is excellent at exploratory work. Neither is excellent at managing hundreds of concurrent, failure-prone, long-running workflows. Gleam (on the BEAM) is.

## The Cost

**Third language.** Three languages means three ecosystems, three deployment targets, three sets of expertise. This is only justified if the orchestration layer is complex enough to warrant it. If pipelines are simple (linear, few customers, rare failures), Tokio handles it fine. If pipelines are complex (many customers, concurrent steps, frequent failures, real-time coordination), the BEAM earns its place.

**Performance on compute-heavy tasks.** The BEAM is not fast for CPU-bound work. It's optimised for concurrency and fault tolerance, not throughput. Data transformations, ML inference, heavy computation — these stay in Rust. The BEAM orchestrates; it doesn't compute.

**Gleam's ecosystem maturity.** Gleam is young. Its standard library is growing but smaller than Elixir's. Some tasks require dropping down to Erlang or Elixir libraries. This is a temporary limitation (the interop is seamless) but it means more time reading Erlang docs than Gleam docs, initially.

## Getting Started

Install Gleam. Build a simple HTTP server with Gleam's Wisp framework. Spawn 10,000 processes, crash half of them, observe the supervision tree restart them. The visceral experience of seeing a system self-heal — without any code you wrote to handle the failure — is what makes the BEAM click. Everything before that moment is theory.

## Reading

- _Designing for Scalability with Erlang/OTP_ — Cesarini & Vinoski (the supervision and process model in depth)
- Joe Armstrong's thesis, "Making Reliable Distributed Systems in the Presence of Software Errors" (2003) — the philosophical foundation of the BEAM, written by Erlang's creator
- Gleam's official language tour (gleam.run/tour) — the syntax and type system in 30 minutes
- Saša Jurić, _Elixir in Action_ — best practical introduction to BEAM concepts, translatable to Gleam
