# WebAssembly as a Plugin System

## The Product Extensibility Problem

The bespoke-to-product transition creates a tension: the product must be general enough for many customers but specific enough for each one. The configuration surface handles the 80% — schema definitions, business rules, output formats. The remaining 20% is genuinely custom logic that doesn't fit any configuration language.

The traditional solutions are all flawed:

**Scripting languages (Python, Lua) embedded in the product.** Flexible but unsafe. Customer code can access the host process's memory, file system, and network. A bad script can crash the product, leak data, or consume unbounded resources. Sandboxing scripting languages is notoriously difficult.

**Webhooks.** Safe (customer code runs elsewhere) but slow. Every invocation is an HTTP round-trip. For hot-path logic that runs per-row in a data pipeline, webhook latency is prohibitive. Also introduces operational complexity: the customer must host and maintain an endpoint.

**Custom code branches per customer.** The thing you're trying to escape. Each branch is bespoke maintenance.

WebAssembly offers a fourth option: customer code that runs inside the product, at near-native speed, with hard sandboxing guarantees.

## What WebAssembly Actually Provides

WASM is a compilation target — a portable bytecode format that any language can compile to. Rust, Go, C, C++, AssemblyScript (TypeScript-like), and others all produce WASM modules. The module runs in a runtime (Wasmtime, Wasmer, WasmEdge) that enforces strict isolation:

**Memory isolation.** A WASM module gets its own linear memory. It cannot access the host's memory. There's no pointer arithmetic that escapes the sandbox. This is enforced at the bytecode level, not by convention.

**Capability-based security.** A WASM module has no capabilities by default — no file system, no network, no environment variables. The host explicitly grants capabilities through imports. If you don't give the module network access, it can't make network calls. This is the principle of least privilege enforced structurally.

**Deterministic execution.** A WASM module given the same inputs produces the same outputs. No access to system time, no randomness (unless the host provides it). This makes plugins testable, reproducible, and cacheable.

**Near-native speed.** WASM compiles to machine code ahead-of-time or just-in-time. Performance is typically within 10-30% of native code. Fast enough for per-row data transformations.

**Resource limits.** The host can limit CPU time (fuel metering in Wasmtime), memory allocation, and stack depth. A runaway plugin gets terminated, not the host.

## The Plugin Architecture

For a data pipeline product, the plugin model looks like:

```
Host (Rust/Axum) defines plugin interface:
  - transform(row: Record) -> Record        // per-row transformation
  - validate(row: Record) -> ValidationResult // custom validation
  - enrich(row: Record) -> Record            // add computed fields
  - decide(features: Features) -> Decision   // custom business logic

Customer compiles their logic to WASM.
Host loads the WASM module, grants only necessary capabilities.
Pipeline calls plugin functions at the appropriate stage.
```

**Extism** is the library that makes this practical. It wraps Wasmtime with a higher-level API for loading plugins, calling functions, managing memory, and passing complex types between host and guest. It has Rust, Python, Go, and Node SDKs. A plugin written in any supported language can be loaded by a host in any supported language.

The Component Model (WASI Preview 2) is the emerging standard for richer plugin interfaces — it defines types, records, enums, and resources that plugins and hosts share through a common IDL (WIT files). This solves the serialisation problem: instead of passing JSON blobs across the boundary, you pass typed records.

## Concrete Applications

**Custom validation rules.** The DV2 pipeline parses data through a strict schema. But some customers have domain-specific validation that doesn't fit the schema language — "this product code must match a pattern that depends on the region" or "these two fields must satisfy a business invariant." A WASM validation plugin runs per-row, returns pass/fail with reasons, and can't corrupt the pipeline.

**Custom transformation logic.** The pricing logic that varies between customers. Instead of hardcoding `if customer == "Speedy"`, the Speedy team ships a WASM module that implements their pricing transform. The product calls it at the right stage. If the module crashes, the pipeline logs the error and continues with a default.

**Feature engineering for ML.** The ML System Architecture describes a shared Rust feature engine. WASM plugins could allow customers to define custom features without modifying the engine. The plugin computes a feature from raw inputs; the engine incorporates it alongside standard features. Domain expertise encoded in portable, sandboxed code.

**Decision rules.** The ML decision plane is currently TypeScript business rules. WASM would allow customers to write decision rules in whatever language they prefer, compiled to a common runtime. Product managers who know Python write in Python. Teams with Rust expertise write in Rust. The host doesn't care.

## The Developer Experience Challenge

WASM plugins are only viable if customers can write them easily. This means:

- A well-documented plugin SDK in at least Rust and Python
- A local testing harness that simulates the host environment
- Example plugins that cover common cases
- Clear error messages when plugins violate constraints (memory limits, capability denials, type mismatches)
- A plugin registry where customers can share and reuse plugins

Shopify's Functions platform is the best reference implementation. They provide a Rust SDK, a local testing tool, TypeScript bindings for convenience, and a deployment pipeline. Study it.

## Risks

**Complexity ceiling.** WASM is still maturing. The Component Model is not yet stable. Debugging WASM modules is harder than debugging native code. The abstraction leaks when you need to pass complex types across the boundary or when performance profiling shows time spent in WASM-to-host calls.

**Language support gaps.** Rust compiles to WASM excellently. Go compiles but with large binary sizes (the runtime is big). Python doesn't compile to WASM natively — you'd need solutions like ComponentizeJS or the experimental Python-to-WASM toolchains, which aren't production-ready. If customers are primarily Python users, the DX story has gaps.

**Adoption friction.** Customers need to learn what WASM is, set up a compilation toolchain, and write code to an interface they've never seen. This is higher friction than a webhook endpoint or a configuration file. The question is whether the 20% of customers who need custom logic are also the 20% who can write code.

## When to Introduce

Not at the start of the product. The WASM plugin system is a maturity indicator — it makes sense when you have enough customers that the 80/20 split is clear, when you've identified the specific extension points that vary between customers, and when you've exhausted what configuration can express. Premature plugin systems are over-engineering. Well-timed plugin systems are the difference between a product that plateaus and one that scales.
