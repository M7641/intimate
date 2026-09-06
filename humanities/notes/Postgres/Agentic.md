
## The Agentic Paradigm Shift

Traditional AI applications are request-response: a user asks, a model answers, the interaction ends. The database requirements are straightforward — store the prompt, store the response, maybe log some metadata.

Agentic AI is fundamentally different. An agent doesn't just answer questions — it **plans, reasons, acts, observes, and iterates**. A single user request might trigger dozens of internal steps: tool calls, database queries, API requests, code execution, memory retrieval, context assembly, reflection, and retry loops. Each step generates data, consumes data, and modifies state — often in unpredictable sequences determined at runtime by the model itself.

This changes everything about what you need from your database layer. The agent is no longer a passive consumer of data — it's an **active, autonomous participant** that reads, writes, searches, and orchestrates data across multiple stores in real time, with latency requirements measured in milliseconds and correctness requirements that cannot be relaxed.

If you get the database architecture wrong, your agent is slow, forgetful, unreliable, and expensive. If you get it right, you have a system that thinks, remembers, learns, and acts with a fluency that feels like magic.

---

## What Makes Agentic Workloads Unique

Before prescribing solutions, it's worth understanding exactly why agentic applications create database challenges that traditional architectures don't anticipate.

### 1. Unpredictable Access Patterns

A traditional web application has predictable database access: user loads page → fetch user profile → fetch content → render. You can optimise indexes, cache strategies, and connection pools around known patterns.

An agent's database access is **determined at runtime by the model**. The agent might decide to search for documents, then look up a user's purchase history, then query an API, then write intermediate results, then search again with a refined query — all within a single task. You cannot predict which tables, which indexes, or which data stores will be hit. Your database layer must be fast for everything, not just the happy path.

### 2. Multi-Store Orchestration

A single agentic step might require data from five different sources simultaneously: vector search for semantic retrieval, relational lookup for structured data, key-value fetch for cached results, graph traversal for relationships, and time-series query for recent events. The agent doesn't care where data lives — it needs the right information, assembled into context, in milliseconds.

This means your database architecture isn't about choosing one database — it's about **orchestrating multiple specialised stores** behind a unified access layer that the agent can call as tools.

### 3. State That Evolves Mid-Task

Traditional applications have clean transaction boundaries: begin, do work, commit. An agent's state evolves continuously over the course of a task that might run for seconds, minutes, or even hours. It maintains a scratchpad of intermediate results, a growing conversation history, a set of observations from tool calls, and a plan that gets revised as new information arrives.

This state needs to be durable (survive crashes), consistent (no stale reads mid-task), and fast (every state read/write is on the critical path of the agent's reasoning loop).

### 4. Memory Across Sessions

Agents need both short-term memory (within a task) and long-term memory (across sessions, days, weeks). Short-term memory is the agent's working context — tool results, intermediate reasoning, conversation history. Long-term memory is what the agent "knows" about the user, the domain, and past interactions.

Long-term memory isn't just "store a JSON blob." It requires **semantic search** (find memories relevant to the current context), **temporal awareness** (recent memories are more relevant), **importance scoring** (not all memories are equal), and **forgetting** (outdated memories should decay). This is a database problem that combines vector search, metadata filtering, and temporal queries.

### 5. Audit, Observability, and Replay

Every action an agent takes needs to be logged, attributed, and replayable. When an agent makes a mistake (and they will), you need to trace exactly what happened: what context was retrieved, what tools were called, what data was read, what decisions were made, and why. This is both a compliance requirement and a debugging necessity.

This generates a high volume of structured event data that needs to be written quickly (not on the agent's critical path) and queried analytically after the fact (what percentage of tool calls failed last week? which retrieval queries returned irrelevant results?).

### 6. Bursty, Variable Load

Agentic workloads are inherently bursty. A single user request might generate 1 database query or 50, depending on the task complexity and the agent's reasoning path. Peak load is unpredictable. You need a database layer that handles spikes gracefully without pre-provisioning for worst-case scenarios.

---

## The Agentic Database Stack

Given these requirements, here's the database architecture that maximises agent performance, reliability, and cost efficiency. Each layer is purpose-built for a specific access pattern that agents generate.

### Layer 1: PostgreSQL — The Operational Brain

**Role:** System of record for structured state, configuration, and transactional data.

PostgreSQL is the foundation of the agentic stack for the same reasons it's the foundation of any serious application: ACID guarantees, sub-millisecond indexed lookups, enforced constraints, and an extension ecosystem that covers half the agent's needs in a single system.

**What lives here:**

- **Agent configuration** — Model parameters, system prompts, tool definitions, routing rules, safety guardrails. Every agent's behaviour is defined by configuration stored in PostgreSQL, editable via the config UI, versioned, and auditable.
    
- **User state and profiles** — Account data, preferences, permissions, subscription tiers. The agent reads this on every request to personalise its behaviour.
    
- **Task state and scratchpad** — The agent's working memory for the current task. Stored as JSONB in a tasks table, updated after each reasoning step. If the agent crashes mid-task, it can resume from the last persisted state.
    
- **Tool registry** — Definitions of every tool the agent can call: name, description, input schema, endpoint, auth requirements, rate limits. The agent's tool-use capability is entirely driven by what's registered here.
    
- **Conversation history (hot)** — Recent conversation turns (last 7–30 days) for fast retrieval. Older history moves to Snowflake.
    
- **Experiment and A/B state** — Which users are assigned to which agent variants, which model versions, which prompt templates. Ground truth for experiment analysis.
    

**Why PostgreSQL wins here:**

- **Sub-millisecond reads** for the config and state lookups that happen on every single agent step. The agent's reasoning loop runs at the speed of its slowest tool — if reading task state takes 200ms instead of 1ms, you've added 200ms to every step of a multi-step chain.
    
- **JSONB for flexible schema** — Agent state is inherently semi-structured and evolves rapidly. JSONB lets you store complex, nested state without schema migrations every time the agent's capabilities change.
    
- **Enforced constraints** — Tool definitions need valid schemas. User permissions need referential integrity. Agent configuration needs validation. PostgreSQL enforces these at the database level, preventing the class of bugs where an agent tries to call a tool that doesn't exist.
    
- **pgvector for moderate-scale vector search** — For many agentic applications, the vector search workload fits comfortably in PostgreSQL via pgvector. Up to ~5–10M vectors, pgvector with HNSW indexes provides sub-10ms approximate nearest-neighbour search. This means you can co-locate your vectors with your relational data, eliminating an entire infrastructure component and the latency of cross-system calls.
    
- **Full-text search built in** — Agents frequently need to search over text: knowledge bases, documentation, conversation histories. PostgreSQL's tsvector/tsquery with pg_trgm for fuzzy matching handles this without Elasticsearch.
    

**The killer feature for agents:** PostgreSQL with pgvector lets you do **hybrid search in a single query** — combine semantic vector similarity with structured metadata filters (date ranges, user IDs, categories, importance scores) in one SQL statement. This is exactly what agent memory retrieval needs, and it's dramatically simpler than coordinating separate vector and relational databases.

```sql
-- Hybrid search: semantic similarity + metadata filtering in one query
SELECT content, metadata, 1 - (embedding <=> $1) AS similarity
FROM agent_memories
WHERE user_id = $2
  AND created_at > NOW() - INTERVAL '30 days'
  AND importance_score > 0.5
ORDER BY embedding <=> $1
LIMIT 10;
```

This query runs in **5–15ms** on pgvector with an HNSW index. Try doing that across two separate databases.

---

### Layer 2: Redis — The Speed Layer

**Role:** Sub-millisecond access for hot data that the agent reads on every step.

Redis (or DragonflyDB for higher throughput) serves as the agent's L1 cache — the data that needs to be available faster than PostgreSQL can serve it, or data that's read so frequently that hitting PostgreSQL would be wasteful.

**What lives here:**

- **Feature vectors for online inference** — Pre-computed user features, item embeddings, and contextual signals that the ML models consume during agent reasoning.
    
- **Session context** — The agent's current working context (assembled prompt, recent tool results, conversation buffer). Stored as a hash or JSON string, keyed by session ID. Read on every reasoning step, updated after each step.
    
- **Rate limit counters** — Per-user, per-tool, per-model rate limits. Agents can be expensive — rate limiting prevents runaway costs. Redis's atomic increment operations make this trivial.
    
- **Tool result cache** — If the agent calls the same tool with the same parameters within a TTL window, serve the cached result instead of re-executing. This is especially valuable for expensive tools (API calls, database queries, code execution).
    
- **Routing and circuit breaker state** — Which model endpoints are healthy, which are degraded, which are down. The agent gateway reads this on every request to route to the best available model.
    

**Why Redis wins here:**

- **< 1ms reads** — The agent's hot loop cannot afford to wait. Redis serves from memory with no query parsing overhead.
    
- **Atomic operations** — Rate limiting, counters, and distributed locks (for preventing duplicate tool execution) are native Redis primitives.
    
- **TTL-based expiry** — Session contexts, cached tool results, and temporary state expire automatically. No garbage collection jobs needed.
    
- **Pub/Sub for real-time events** — When configuration changes (new tool registered, model swapped, guardrail updated), Redis pub/sub propagates the change to all agent instances in milliseconds.
    

---

### Layer 3: Dedicated Vector Store — The Long-Term Memory (At Scale)

**Role:** Semantic search over large-scale knowledge bases and memory stores.

For applications where the vector corpus exceeds what pgvector handles comfortably (~10M+ vectors), or where you need advanced features like multi-tenancy, hybrid search with facets, or real-time index updates at high write throughput, a dedicated vector database becomes necessary.

**Options and trade-offs:**

|Store|Strength|Best for|
|---|---|---|
|**pgvector** (PostgreSQL)|Co-located with relational data; hybrid search in one query; zero additional infrastructure|< 10M vectors; moderate query throughput; when hybrid search is primary pattern|
|**Qdrant**|Rust-native, fast, excellent filtering, payload storage|Production RAG systems needing high throughput and advanced filtering|
|**Weaviate**|GraphQL API, multi-modal, built-in vectorisation|Teams wanting managed vectorisation and a higher-level API|
|**Pinecone**|Fully managed SaaS, auto-scaling, zero ops|Teams that want zero infrastructure management|
|**Milvus**|Open source, GPU-accelerated, massive scale|100M+ vectors, teams with GPU infrastructure|

**What lives here:**

- **Knowledge base embeddings** — Documents, articles, FAQs, product descriptions, code repositories — everything the agent might need to retrieve as context.
    
- **Long-term memory embeddings** — Vectorised memories from past interactions, indexed for semantic retrieval. The agent searches these on every new task to recall relevant prior context.
    
- **Tool documentation embeddings** — Descriptions and examples of how to use each tool, so the agent can semantically match a user's request to the right tool.
    

**The critical insight for agents:** The vector store isn't just a search engine — it's the agent's **episodic memory**. The quality of what the agent retrieves directly determines the quality of what it produces. Invest in your retrieval pipeline (chunking strategy, embedding model, re-ranking) at least as much as you invest in your generation model.

---

### Layer 4: Snowflake — The Analytical Brain

**Role:** Historical analysis, training data, observability, and business intelligence over agentic operations.

Snowflake sits entirely off the agent's critical path. No agent reasoning step should ever query Snowflake directly. Instead, Snowflake is where you go to **understand, improve, and govern** your agentic system.

**What lives here:**

- **Complete agent trace logs** — Every reasoning step, every tool call, every retrieval query, every model response, with full payloads. This is your forensic record. When an agent makes a bad decision, you trace back through the log to understand why.
    
- **Retrieval quality analytics** — For every retrieval query the agent made, what was retrieved? Was it relevant? Did the agent use it? Snowflake lets you run analytical queries over millions of retrieval events to identify patterns: which knowledge base chunks are never retrieved, which queries return irrelevant results, where the chunking strategy is failing.
    
- **Tool usage analytics** — Which tools are called most? Which fail most? What's the average latency per tool? Are there tools the agent never uses (maybe remove them to reduce confusion)? Are there tools it overuses (maybe add guardrails)?
    
- **Model performance comparison** — A/B experiment analysis. Did agent variant A (GPT-4o) produce better outcomes than variant B (Claude Sonnet) for customer support tasks? Join experiment assignments with outcome events (task completion, user satisfaction, escalation rate) to answer this.
    
- **Cost attribution** — How much did each agent task cost? Break down by model inference, tool execution, retrieval queries, and compute. Snowflake's analytical power lets you build detailed cost models: cost per task, cost per user, cost per tool, cost per resolution.
    
- **Training data curation** — The best fine-tuning data comes from your own agents' successful interactions. Snowflake is where you identify high-quality agent traces (tasks with positive user feedback, no errors, efficient tool usage) and export them as training examples.
    
- **Memory management analytics** — Which long-term memories are accessed most? Which have decayed in relevance? Snowflake powers the batch jobs that prune, consolidate, and re-rank the agent's memory store.
    

**Why Snowflake wins here:**

- **Petabyte-scale trace storage** — Agentic systems generate enormous volumes of trace data. A single agent task might produce 50+ log events. Multiply by millions of tasks per day and you're in terabyte territory within weeks. Snowflake handles this without breaking a sweat.
    
- **Elastic compute for ad-hoc analysis** — When a product manager asks "why did agents escalate 40% more tickets last Tuesday?", you need to scan millions of trace records with complex joins. Snowflake spins up a warehouse, runs the query in seconds, and shuts down.
    
- **Zero-copy data sharing** — Share agent performance data with partner teams, customers, or auditors without moving data. Particularly valuable in enterprise agentic deployments where governance requires visibility.
    
- **Native ML integration** — Snowpark and Cortex let you run Python-based analysis (embedding quality evaluation, clustering of failure modes, anomaly detection on agent behaviour) directly on the warehouse.
    

---

### Layer 5: The Graph Layer (When You Need It)

**Role:** Relationship-aware reasoning and multi-hop knowledge traversal.

Not every agentic application needs a graph database, but the ones that do really need it. If your agent reasons over entities with complex relationships — organisational hierarchies, product dependencies, knowledge graphs, social networks, causal chains — a graph layer transforms what's possible.

**Options:**

|Store|Strength|
|---|---|
|**Neo4j**|Mature, Cypher query language, excellent for traversals|
|**Apache AGE** (PostgreSQL extension)|Graph queries within PostgreSQL — no additional infrastructure|
|**Amazon Neptune**|Managed, good for RDF and property graphs|

**What the graph layer enables for agents:**

- **Multi-hop reasoning** — "Find all customers who bought product X, then find what else they bought, then find which of those products had complaints, then summarise the complaint themes." This is 4 hops across 3 entity types — trivial in a graph, painful in SQL.
    
- **Dependency resolution** — "What services depend on this API? If I change the schema, what breaks?" The agent traverses the dependency graph to assess impact.
    
- **Knowledge graph-augmented retrieval** — Instead of pure vector search ("find documents similar to this query"), the agent first identifies relevant entities in the knowledge graph, then retrieves documents connected to those entities. This produces dramatically more precise retrieval for domain-specific tasks.
    
- **Causal reasoning** — "What caused this incident?" The agent traverses a causal graph of events, dependencies, and changes to identify root causes.
    

**When to add it:** Start without a graph database. If your agent's reasoning quality is limited by its ability to traverse relationships (you'll know because vector search keeps returning tangentially relevant results, or the agent struggles with "how does X relate to Y" questions), then add a graph layer. Apache AGE within PostgreSQL is the lowest-friction starting point.

---

## How the Agent Interacts With the Database Stack

Here's the critical architectural pattern: the agent doesn't talk to databases directly. It talks to **tools**, and each tool encapsulates a database interaction with proper error handling, caching, and observability.

```
User request arrives
    │
    ▼
Agent Gateway (reads config from PostgreSQL, session from Redis)
    │
    ▼
Agent Reasoning Loop
    │
    ├── Step 1: Retrieve context
    │   └── Tool: memory_search
    │       ├── Searches pgvector (hybrid: semantic + metadata filter)
    │       ├── Searches vector store (if scale requires it)
    │       └── Returns ranked, deduplicated context
    │
    ├── Step 2: Look up structured data
    │   └── Tool: database_query
    │       ├── Executes parameterised SQL against PostgreSQL
    │       ├── Checks Redis cache first
    │       └── Returns structured results
    │
    ├── Step 3: Call external API
    │   └── Tool: api_call
    │       ├── Checks rate limit (Redis)
    │       ├── Checks result cache (Redis)
    │       ├── Executes API call
    │       └── Caches result in Redis with TTL
    │
    ├── Step 4: Write intermediate result
    │   └── Tool: scratchpad_write
    │       ├── Updates task state in PostgreSQL (JSONB)
    │       └── Updates session context in Redis
    │
    ├── Step 5: Reason and decide next action
    │   └── Model inference (reads assembled context)
    │
    ├── ... (loop continues until task complete)
    │
    └── Final: Return result to user
        ├── Write final state to PostgreSQL
        ├── Log full trace to NATS → Snowflake
        ├── Update long-term memory (pgvector / vector store)
        └── Clear session from Redis
```

**Every database interaction is wrapped in a tool with:**

- Input validation (prevent SQL injection, enforce schemas)
- Timeout handling (agent can't hang on a slow query)
- Retry logic (transient failures shouldn't crash the task)
- Caching (Redis-first for hot data)
- Observability (every tool call is logged with latency, result size, cache hit/miss)
- Cost tracking (each database query has an estimated cost that feeds into task-level cost attribution)

---

## Database Selection Decision Framework for Agentic Applications

### Start Here (Day 1)

**PostgreSQL + pgvector + Redis.** That's it. Three components.

PostgreSQL handles structured state, configuration, conversation history, vector search (up to ~5–10M vectors), full-text search, and JSON document storage. Redis handles session state, caching, rate limiting, and real-time events. This stack covers 80% of agentic applications.

You do not need Pinecone, Weaviate, Qdrant, Neo4j, Elasticsearch, MongoDB, or Snowflake on day one. Every additional database is a new failure mode, a new connection to manage, a new consistency boundary to reason about, and a new cost centre. Start simple. Add complexity when you have evidence that you need it.

### Scale Triggers (When to Add More)

|Signal|Action|
|---|---|
|Vector corpus > 10M embeddings|Evaluate dedicated vector store (Qdrant, Milvus)|
|Agent trace volume > 100GB|Add Snowflake for analytical queries over traces|
|Need multi-hop relationship reasoning|Add Apache AGE (PostgreSQL extension) or Neo4j|
|> 50 concurrent agent tasks hitting PostgreSQL|Add read replicas; consider Citus for horizontal scaling|
|Dashboard/BI queries competing with agent queries|Add Snowflake; move analytical workloads off PostgreSQL|
|Need to share agent performance data cross-org|Snowflake data sharing|
|Real-time streaming events (> 100K events/sec)|NATS JetStream or Kafka as event bus|

### Anti-Patterns to Avoid

|Anti-Pattern|Why it fails|What to do instead|
|---|---|---|
|Agent queries Snowflake during reasoning|200ms+ minimum overhead per query; kills agent responsiveness|Pre-materialise to PostgreSQL/Redis; Snowflake is for batch only|
|Separate vector DB + relational DB for hybrid search|Cross-system latency; consistency challenges; operational overhead|Use pgvector in PostgreSQL for hybrid search in a single query|
|Storing all conversation history in PostgreSQL forever|Table bloats; query performance degrades|Hot/warm/cold: 30 days in PostgreSQL, older in Snowflake|
|No caching layer (hitting PostgreSQL for every tool call)|Unnecessary load; repeated identical queries waste latency|Redis cache with TTL for tool results, config, and session state|
|Using MongoDB "because it's flexible"|No ACID for task state; no vector search; no full-text search; no relational joins; adds a database with fewer capabilities than PostgreSQL|PostgreSQL with JSONB gives you document flexibility + everything else|
|Storing agent traces in PostgreSQL|High write volume; analytical queries compete with agent queries; vacuum pressure|Write to NATS → Snowflake; keep only hot traces (7 days) in PostgreSQL|
|One giant vector index with no metadata filtering|Retrieval quality degrades; irrelevant results from other users/domains|Partition vectors by tenant/domain; use pgvector's hybrid search with WHERE clauses|
|No rate limiting on agent tool calls|Runaway agents can generate thousands of database queries in seconds|Redis-based rate limiting per tool, per user, per time window|

---

## The Competitive Edge: Why This Stack Wins

### Speed

The agent's reasoning loop runs at the speed of its slowest step. With PostgreSQL (sub-millisecond indexed lookups), Redis (sub-millisecond cache), and pgvector (5–15ms semantic search), your agent's tool calls complete in **single-digit milliseconds** for cached data and **low double-digit milliseconds** for retrieval. Compare this to architectures where every tool call crosses a network boundary to a separate vector database (add 10–50ms), a separate document store (add 10–30ms), and a separate metadata store (add 5–20ms). Over a 10-step agent chain, those milliseconds compound into seconds of additional latency.

### Simplicity

Three components (PostgreSQL, Redis, NATS) instead of seven (PostgreSQL, Redis, Pinecone, Elasticsearch, MongoDB, Kafka, Snowflake). Fewer components means fewer failure modes, fewer consistency boundaries, fewer connection pools, fewer monitoring dashboards, and fewer things to explain to your on-call engineer at 3am. PostgreSQL's extension ecosystem (pgvector, pg_trgm, PostGIS, JSONB) consolidates capabilities that would otherwise require separate systems.

### Cost

Every additional database service has a base cost regardless of usage. A dedicated vector database, even at small scale, costs £50–500/month. Elasticsearch for text search adds another £100–500/month. A graph database adds more. PostgreSQL with pgvector and pg_trgm does vector search, text search, and relational storage for the cost of a single managed PostgreSQL instance (£15–200/month depending on size). At startup scale, this difference is significant. At enterprise scale, the operational cost savings of fewer systems to manage dwarfs the infrastructure cost.

### Observability

When everything flows through PostgreSQL and NATS, you have a unified view of the agent's data interactions. Every tool call, every retrieval query, every state update is logged through a consistent pipeline. When something goes wrong, you don't need to correlate logs across five different databases — the trace is in one analytical store (Snowflake) fed from one event bus (NATS).

### Evolvability

Starting with PostgreSQL + pgvector doesn't lock you in. When you outgrow pgvector, you add a dedicated vector store alongside it — pgvector continues serving hybrid search while the dedicated store handles pure semantic search at scale. When you need graph queries, Apache AGE adds graph capabilities within PostgreSQL. When you need analytics, Snowflake is added off the critical path. The architecture grows incrementally, each addition solving a specific, measured bottleneck.

---

## The Full Picture

```
┌──────────────────────────────────────────────────────────────┐
│                    AGENT REASONING LOOP                       │
│                                                              │
│   Plan → Act → Observe → Reflect → (repeat)                 │
│                                                              │
│   Each step calls tools. Each tool hits the right store.     │
└──────────────┬───────────────────────────────────────────────┘
               │
    ┌──────────┼───────────────────────────────┐
    │          │ TOOL LAYER                     │
    │  ┌───────┴────────┐  ┌────────────────┐  │
    │  │ memory_search   │  │ database_query │  │
    │  │ scratchpad_rw   │  │ api_call       │  │
    │  │ graph_traverse  │  │ code_execute   │  │
    │  └───────┬────────┘  └───────┬────────┘  │
    └──────────┼───────────────────┼───────────┘
               │                   │
    ┌──────────▼───────────────────▼───────────────────────┐
    │              DATA LAYER                               │
    │                                                       │
    │  ┌─────────────────────────────────────────────┐     │
    │  │         PostgreSQL + pgvector                │     │
    │  │  • Structured state    • Vector search       │     │
    │  │  • Config & tools      • Full-text search    │     │
    │  │  • Task scratchpad     • Conversation history │     │
    │  │  • User profiles       • Experiment state     │     │
    │  └─────────────────────────────────────────────┘     │
    │                                                       │
    │  ┌──────────────┐  ┌──────────────┐                  │
    │  │    Redis      │  │    NATS      │                  │
    │  │  • Session    │  │  • Events    │                  │
    │  │  • Cache      │  │  • Traces    │                  │
    │  │  • Rate limit │  │  • Config Δ  │                  │
    │  └──────────────┘  └──────┬───────┘                  │
    │                           │                           │
    └───────────────────────────┼───────────────────────────┘
                                │
    ┌───────────────────────────▼───────────────────────────┐
    │            ANALYTICAL LAYER (off critical path)        │
    │                                                       │
    │  ┌─────────────────────────────────────────────┐     │
    │  │              Snowflake                       │     │
    │  │  • Agent trace analysis   • Cost attribution │     │
    │  │  • Retrieval quality      • A/B experiments  │     │
    │  │  • Training data curation • Memory analytics │     │
    │  └─────────────────────────────────────────────┘     │
    │                                                       │
    └───────────────────────────────────────────────────────┘

    ┌───────────────────────────────────────────────────────┐
    │         SCALE-UP ADDITIONS (when needed)               │
    │                                                       │
    │  ┌──────────────┐  ┌──────────────┐                  │
    │  │ Qdrant/Milvus │  │ Neo4j/AGE    │                  │
    │  │ (10M+ vectors)│  │ (graph       │                  │
    │  │              │  │  reasoning)  │                  │
    │  └──────────────┘  └──────────────┘                  │
    │                                                       │
    └───────────────────────────────────────────────────────┘
```

---

## The Sales Pitch, Distilled

If you're building agentic applications, here's the uncomfortable truth: **most of the "AI infrastructure" being sold to you is premature optimisation.**

You don't need a dedicated vector database until you have 10 million vectors. You don't need a graph database until your agent's reasoning is bottlenecked by relationship traversal. You don't need Elasticsearch until PostgreSQL's built-in search can't keep up. You don't need Snowflake until your trace data outgrows PostgreSQL.

What you need on day one is **PostgreSQL with pgvector, Redis, and NATS.** Three components. One relational database that doubles as your vector store, document store, and search engine. One cache that makes your agent's hot loop sub-millisecond. One event bus that decouples your critical path from your analytical pipeline.

This stack serves a single agent task in **under 50ms of database time** across a 10-step reasoning chain. It handles hundreds of concurrent agent sessions on a single PostgreSQL instance. It costs less than £200/month in managed infrastructure at startup scale.

When you grow — and you will — you add Snowflake for analytics (off the critical path), a dedicated vector store for scale (alongside pgvector, not replacing it), and a graph layer for complex reasoning (if and only if you need it). Each addition is justified by a measured bottleneck, not a vendor's pitch deck.

The agents of the future won't be differentiated by which LLM they use — that's rapidly commoditising. They'll be differentiated by **the quality, speed, and intelligence of their data layer.** Build that layer right, and everything else follows.