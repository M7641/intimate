# Durable Execution — Temporal, Restate, and the Problem of Long-Running Work

## The Problem Nobody Talks About

The agentic architecture describes a five-layer stack: PostgreSQL for state, Redis for speed, vector store for memory, Snowflake for analytics, NATS for messaging. It handles the _structure_ of agentic systems well. What it doesn't address is the _lifecycle_ of a single agentic workflow.

Consider a realistic pipeline: receive a data file → validate schema → enrich from external API → run ML inference → apply business rules → write results → notify customer. Six steps. Each can fail independently. The external API might time out. The ML service might be overloaded. The customer notification endpoint might be down.

In a naive implementation, failure at step 4 means restarting from step 1. This wastes work and can cause side effects (re-enriching from a rate-limited API, double-writing results). The standard mitigation is checkpointing — save state after each step, resume from the last checkpoint on failure. But manual checkpointing is the "shotgun validation" of orchestration: you repeat the same pattern everywhere, get it subtly wrong in each place, and spend more time managing state than doing useful work.

Durable execution solves this structurally. You write sequential code. The runtime makes it durable.

## How Temporal Works

Temporal separates _what to do_ (your code) from _how to survive failure_ (the runtime).

**Workflows** are functions that describe the overall process. They look like normal sequential code — call this, then call that, then decide based on the result. But the Temporal runtime persists every step to a database. If the process dies, the runtime replays the workflow from the persisted history, skipping completed steps and resuming from the last incomplete one.

**Activities** are the individual steps — the things that interact with the outside world (API calls, database writes, file processing). Activities can fail, time out, and retry. Temporal manages the retry policy, backoff, and timeout for each activity independently.

**Workers** are the processes that execute workflows and activities. They're stateless — they pull work from Temporal's task queues and execute it. If a worker dies, another picks up the work. Scaling means adding workers.

The key insight: the workflow function is deterministic. Given the same inputs and the same history of completed activities, it produces the same sequence of next steps. This is what enables replay — Temporal replays the workflow, feeds it the recorded results of completed activities, and the workflow deterministically arrives at the next pending activity.

```
// Pseudocode — this looks like normal sequential code
async fn process_data(file: File) -> Result<Report> {
    let validated = activity::validate_schema(file).await?;
    let enriched = activity::enrich_from_api(validated).await?;
    let predictions = activity::run_inference(enriched).await?;
    let results = activity::apply_rules(predictions).await?;
    activity::write_results(results).await?;
    activity::notify_customer(results.summary()).await?;
    Ok(results.into_report())
}
```

If the process crashes after `enrich_from_api` completes but before `run_inference` starts, Temporal replays: it sees `validate_schema` completed (skips), sees `enrich_from_api` completed (skips), and calls `run_inference`. No re-enrichment. No checkpoint code. The durability is invisible.

## Restate — The Lightweight Alternative

Restate takes the same core idea but makes it lighter. Instead of a separate Temporal cluster (which is a significant piece of infrastructure), Restate is a single binary that sits between your services and makes their function calls durable.

You write HTTP handlers. Restate intercepts the calls, journals every step, and replays on failure. It feels more like middleware than an orchestration platform.

Restate's advantage: lower operational overhead, faster to adopt, works well with existing HTTP services. Its disadvantage: less mature, smaller ecosystem, fewer features (no advanced scheduling, signals, or queries that Temporal provides).

For a team already running Axum services with NATS for messaging, Restate integrates more naturally than Temporal. It's a library, not a platform.

## Where This Solves Real Problems

**Data pipeline recovery.** The DV2 ingestion pipeline (parse → validate → hash → hub/link/satellite → transform) is a multi-step process where failure at any step currently means restarting. With durable execution, a schema validation failure at step 2 doesn't re-parse the file. A hub insert failure doesn't re-validate. Each step's completion is guaranteed.

**Agentic workflows with external dependencies.** An agentic pipeline that calls an LLM, then an external API, then writes to a database involves three unreliable external systems. Durable execution handles retries, timeouts, and partial completion without any manual state management.

**Long-running processes with human decisions.** Some workflows need human input mid-stream — approval of a pricing change, validation of a data anomaly, confirmation of a model retraining trigger. Temporal natively supports "signals" (external events injected into a running workflow) and can pause a workflow for days or weeks waiting for human input, then resume exactly where it left off.

**Saga pattern for distributed transactions.** When a multi-step process needs to be all-or-nothing across multiple services (write to PostgreSQL, update Snowflake, notify customer), durable execution implements the saga pattern naturally: each activity has a compensating activity that undoes it on failure. Temporal manages the compensation chain automatically.

## The Cost

**Determinism requirement.** Workflow code must be deterministic — no random numbers, no current time, no non-deterministic operations. These must be done in activities, not workflows. This is a real constraint that requires discipline.

**Infrastructure.** Temporal requires a separate cluster (PostgreSQL or Cassandra-backed). Restate is simpler (single binary) but still adds a component. Neither is zero-cost to operate.

**Debugging complexity.** When a workflow behaves unexpectedly, you need to understand the replay model. The Temporal Web UI helps, but reasoning about replayed execution is inherently harder than reasoning about sequential execution.

**Vendor coupling.** Temporal's SDK shapes how you structure code. Migrating away from it means restructuring orchestration logic. Restate is lighter in this regard but still introduces coupling.

## When to Adopt

Start with the messiest multi-step pipeline — the one that fails most often and requires the most manual intervention to recover. Wrap it in a Temporal workflow (or Restate handler) and see if the recovery behaviour alone justifies the infrastructure cost. If it does, expand. If it doesn't, the pipelines are simpler than you think.

The threshold: if you have more than three retryable steps in sequence and you're currently handling failure by restarting from the beginning or by manual intervention, durable execution pays for itself.

## Reading

- Temporal's "Why Durable Execution?" docs (conceptual, not sales-heavy)
- Restate's documentation (particularly the "Journal" concept, which explains the replay model clearly)
- The original Sagas paper by Hector Garcia-Molina (1987) — durable execution is essentially Sagas with a modern runtime
- Uber's Cadence (Temporal's predecessor) engineering blog posts — the real-world motivation
