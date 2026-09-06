Every capability starts as something one person figures out for one situation. If it works, others want it. If enough others want it, someone packages it. If enough people use the package, it disappears into infrastructure. This is the journey from bespoke to product to commodity, and understanding it changes how you make decisions about what to build, what to buy, and what to let go.

This note goes deep because the transition between stages is where most teams get stuck — and where most value is created or destroyed.

---

## Stage 1: Bespoke (Genesis → Custom-Built)

### What It Looks Like

Someone has a problem. No existing tool solves it. So they build something — messy, specific, deeply coupled to the context it was born in. It works for that situation, maybe brilliantly, but it's held together by the builder's understanding of both the problem and the solution. It lives in their head as much as in the code.

This is a single client's world — eight facets, each a bespoke response to a specific need: pricing, bidding, quoting, audiences, catalogue, tiers, data movement, pipeline. None of them are general. All of them depend on context that isn't written down.

This is also the early days of a data pipeline, the first customer-specific data transformations, the first ML model built for a specific use case. Everything starts here.

### Why It's Valuable

Bespoke work is where learning happens. You can't build a product for a domain you don't understand, and you can't understand a domain without doing the bespoke work first. A recommender for one client, transaction hubs for another, a supply/demand model for a third — each one teaches something about what's general and what's specific. That learning is the raw material for everything that follows.

The mistake isn't doing bespoke work. The mistake is staying there.

### Why Teams Get Stuck Here

Three forces keep work bespoke:

**The hero trap.** Bespoke work creates heroes — the person who understands the system because they built it. Heroes are rewarded (they're indispensable) and punished (they can never leave). The organisation depends on their knowledge, which means the organisation depends on the work staying bespoke, because generalising it would distribute the knowledge and reduce the hero's centrality. This isn't malice; it's incentive structure.

**The customer trap.** Each customer believes their situation is unique. They're usually 20% right and 80% wrong. The 20% that's genuinely unique gets used to justify keeping 100% custom. "But our pricing logic is different" becomes the reason the entire pipeline is hand-built, even though 80% of it (data ingestion, validation, transformation, serving) is identical to every other customer.

**The revenue trap.** Bespoke work is billable immediately. Productising requires investment now for returns later. When the team is small and revenue pressure is real, there's always a reason to take the next custom project instead of investing in the platform. Each custom project makes the next one slightly easier (reused code, pattern recognition) but never crosses the threshold into an actual product. It's the trap of trying to sell something you don't do, because what you do (bespoke) is what pays.

### How to Know It's Time to Leave

Signals that bespoke has served its purpose:

- You've built the same thing three or more times with superficial variations
- You can predict what a new customer will need before they tell you
- The differences between implementations are in configuration, not architecture
- Support burden is growing faster than revenue
- The builder's mental model is more complete than any documentation, and that person is a bottleneck

A maturing pipeline shows these signals clearly. The pattern (ingest → parse → hash → hub/link/satellite → transform → serve) is the same across customers. The variations are in schemas, business rules, and output shapes. That's configuration, not architecture.

---

## Stage 2: The Transition — Bespoke to Product

This is the hard part. Not because the engineering is difficult, but because it requires a fundamentally different way of thinking. Building bespoke means solving the problem in front of you. Building product means solving the *category* of problem — including cases you haven't seen yet.

### What Changes

**From "what does this customer need" to "what do all customers need."** This sounds obvious but it's a deep shift. It means looking at your implementations and separating the invariant (the parts that are always the same) from the variant (the parts that change). The invariant becomes the product. The variant becomes configuration.

For a data pipeline: the invariant is the flow (ingest → validate → transform → store). The variant is the schema (what columns, what types, what business rules). The product is an engine that accepts schema definitions and executes the invariant flow. The configuration is the schema itself.

**From implicit knowledge to explicit contracts.** In bespoke mode, the builder knows what the system expects. In product mode, the system must declare what it expects — through schemas, API contracts, validation rules, error messages. This is the wall strategy made concrete. The product defines its walls; customers interact through them.

Validation through parsing is the design philosophy for this transition. Bespoke systems validate loosely (check what you remember to check). Products parse strictly (the input either conforms or it doesn't). The parser *is* the wall.

**From code to configuration.** The most painful part. Every `if customer == "Acme"` in the codebase is a failure to productise. Every hardcoded path, magic number, customer-specific branch — these are bespoke thinking embedded in what's pretending to be a product. Removing them requires identifying the actual degrees of freedom and making them configurable.

This doesn't mean making everything configurable. That's the other trap — the "infinitely flexible platform" that's so configurable it's unusable. The discipline is identifying the *right* configuration surface: large enough to cover real variation, small enough to be comprehensible.

### The Extraction Pattern

A practical approach for going from multiple bespoke implementations to a product:

**Step 1: Catalogue the implementations.** List every customer implementation of the same capability. For each one, document: what it does, how it does it, what's customer-specific, what's shared.

**Step 2: Find the invariant.** Look at the shared parts. If all implementations ingest data, validate it, transform it, and serve it — that sequence is the product. The specific validations, transformations, and serving formats are configuration.

**Step 3: Define the configuration surface.** What are the actual knobs? For a data pipeline: source schema definition, validation rules, transformation logic, output format. Each of these becomes a configuration type — a schema for schemas, a rule engine for validation, a transformation DSL for logic.

**Step 4: Build the engine.** The product is the engine that reads configuration and executes the invariant flow. The engine should have no knowledge of any specific customer. If you have to name a specific customer anywhere in the engine code, you've failed.

**Step 5: Migrate one customer.** Take the simplest existing implementation and express it as configuration for the new engine. This is the acid test. If the configuration can't express what the bespoke implementation does, the configuration surface is too small. If the engine can't execute the configuration correctly, the engine has bugs. Fix both.

**Step 6: Migrate the rest.** Each migration validates the product and often reveals missing configuration options. The product grows through real usage, not speculation.

The early stages (1-3) are often already visible in the existing work — schemas, validation, tiers, and partner-enablement thinking work through the configuration surface. The gap is steps 4-6: building the engine and proving it works by migrating real customers onto it.

### What You Lose

Productising has real costs:

**Speed for specific customers.** A bespoke solution built for one customer can be faster to deliver than configuring a product. The product pays off over time, not on the first use. This means the first customer on the product will have a worse experience than they would have had with a bespoke solution. You need to be honest about this trade-off rather than pretending the product is immediately better.

**Edge case coverage.** Products serve the 80% case well. The 20% that's genuinely customer-specific either gets handled through extension points (plugins, custom transforms, webhooks) or gets declined. Declining is hard but necessary — every edge case you absorb into the product makes it more complex for everyone.

**The feeling of craftsmanship.** There's satisfaction in building something perfectly fitted to one situation. Products are compromises by design. The satisfaction shifts from "I built exactly what they needed" to "I built something that 50 customers use without needing me." Different reward, but it scales.

### What You Gain

**Time.** Not immediately, but compounding. The tenth customer on a product takes a fraction of the time the tenth bespoke customer would have taken.

**Quality.** The product gets every bug fix, every performance improvement, every refinement. Bespoke implementations stagnate the moment you move to the next customer. This is why "let them rot" is tempting advice for bespoke systems — they inevitably do. Products don't have to.

**Teachability.** New team members can learn a product. They can't learn ten bespoke systems. Conway's Law in action: the product creates an organisation that can grow, because knowledge is in the system rather than in individuals.

**Arguability.** "We can't do that because our product doesn't support it" is a stronger position than "we can't do that because we don't have time." The product creates boundaries that are structural, not personal. The wall is the architecture, not your calendar.

---

## Stage 3: Product (Established)

### What a Mature Product Looks Like

A useful maturity model sketches this across several dimensions. A mature product has:

**A clear boundary.** It does one thing well. Quote-to-Order, Inventory, Merchandising — each is a product, not a department. The boundary is enforced by the interface (API, schema, event contract), not by convention.

**A configuration language.** Customers interact through configuration, not code. The configuration is expressive enough to cover the 80% case and has escape hatches (webhooks, custom transforms, extensions) for the remaining 20% that justify the complexity.

**Observable behaviour.** The observability stack exists to answer: "Is the product working? For which customers? Where is it slow? Where is it wrong?" This is the tiered alerting strategy (immediate / weekly / queryable) applied at the product level. Every customer's instance is monitored through the same dashboard.

**A feedback loop.** Customer usage data flows back into product development. The most-used configuration options get optimised. The most-requested missing features get prioritised. Bespoke work has no feedback loop because each implementation is isolated.

**Documentation as interface.** The product's documentation is part of its surface area. Not an afterthought, not a wiki no one reads — it's how customers (and new team members) understand what the product does, what it doesn't do, and how to configure it. Good technical writing connects here directly.

### The Product Plateau

Many products reach maturity and stop. They work, they have customers, they generate revenue. But they never become commodities. Most shouldn't — not every product needs to become infrastructure. But understanding why some do and some don't illuminates what's actually happening.

Products plateau when:

- The market is too small for commoditisation (niche vertical solutions)
- The configuration surface is too complex for self-service (customers still need consultants)
- The competitive landscape is fragmented (many similar products, none dominant)
- The team protects the product from competition instead of racing ahead of it

The question for each product boundary (Quote→Order, Inventory, Merchandising) is: should this become a commodity, or is it more valuable as a differentiated product? The answer depends on whether the value is in the *engine* (which can commoditise) or in the *configuration knowledge* (which stays as service).

---

## Stage 4: The Transition — Product to Commodity

This transition is different from bespoke-to-product. It's not about extraction and generalisation; it's about *evaporation*. The product becomes so standard, so reliable, so universally expected that it disappears into the background. Nobody thinks about electricity as a product. Nobody evaluates database hosting as a differentiator (for most companies). These things commoditised.

### What Drives Commoditisation

**Standardisation of interfaces.** When every product in a category uses the same API shape, the same data format, the same protocol — switching between them becomes trivial. OpenTelemetry is commoditising observability interfaces. SQL commoditised query languages. REST/gRPC commoditised API patterns. S3's API commoditised object storage (even competing providers implement the S3 API).

**Operational maturity.** When running the thing becomes boring. No surprises, no midnight pages, no special knowledge required. Managed PostgreSQL (RDS, Cloud SQL, Neon) is PostgreSQL commoditised. You don't operate it; you configure it and it runs.

**Economic pressure.** When customers stop paying premium prices for the capability because alternatives are good enough. This is the signal that differentiation has evaporated. If every data pipeline tool can ingest, validate, and transform with roughly the same quality, the pipeline itself is a commodity. The value moves elsewhere — to the configuration knowledge, the domain expertise, the speed of deployment.

**Open source as accelerant.** Open source doesn't cause commoditisation, but it accelerates it dramatically. PostgreSQL is a commodity because it's free and excellent. Kubernetes is a commodity because it's free and ubiquitous. When your product competes with a free alternative that's good enough, you're already in commodity territory whether you admit it or not.

### What Happens to Value

When something commoditises, value doesn't disappear — it migrates. It moves up the stack to higher-level capabilities that are still in the product or bespoke stage.

When databases commoditised, value moved to application logic. When cloud infrastructure commoditised, value moved to the services built on it. When ML frameworks commoditised (PyTorch, TensorFlow), value moved to the data and feature engineering.

For the current landscape: if data pipelines commoditise (and tools like Fivetran, Airbyte, dbt are pushing in that direction), value migrates to what happens *after* the data moves — the models, the decisions, the domain-specific logic. This is why ML system architecture matters: it's where value goes when the plumbing becomes boring.

The strategic question is always: what layer of the stack is commoditising beneath you, and are you building on top of it or competing with it?

---

## Stage 5: Commodity (Utility)

### What a Commodity Looks Like

Invisible. Nobody thanks the electricity company for keeping the lights on. Nobody congratulates PostgreSQL for not losing data. Commodities are expected to work and noticed only when they fail.

Characteristics:

- **Pay-per-use or flat-rate pricing.** No sales calls, no custom quotes. S3 costs $0.023 per GB per month. Done.
- **Self-service.** No onboarding needed. Sign up, configure, use. If a commodity requires a consultant, it's still a product.
- **API-first.** Everything is programmable. Humans don't operate commodities; systems do.
- **Extreme reliability.** Five nines (99.999%) is the expectation. Failure is newsworthy.
- **Zero competitive differentiation on the capability itself.** All object stores store objects. All managed databases run databases. Competition moves to price, ecosystem, and adjacent services.

### Operating in the Commodity Layer

The commonly settled commodities are clear: PostgreSQL, Snowflake, Redis, NATS, Kubernetes, S3, Grafana. The principle is clear — use these, don't build alternatives, and don't try to differentiate on them.

But there's a subtlety: the *operational knowledge* of commodities is still valuable even when the capabilities themselves aren't. Knowing how to tune PostgreSQL for agentic workloads, knowing how to compress Redshift columns optimally, knowing when to choose PostgreSQL vs Snowflake — this is expertise on top of commodity infrastructure. It's valuable precisely because the commodity is ubiquitous. Everyone uses PostgreSQL, but not everyone uses it well.

This is a viable business position: deep expertise on commodity infrastructure, applied to specific domains. You're not selling the database; you're selling the knowledge of how to use it for pricing optimisation, inventory management, or supply chain decisions.

---

## The Full Journey — Applied to the Current Stack

Mapping this to the current landscape:

**Currently bespoke (should be moving to product):**
- Customer-specific data pipelines (one per client)
- Customer-specific ML models and decision logic
- Customer-specific reporting and dashboards

**Currently transitioning to product:**
- The ingestion engine (schemas, validation, partner enablement show the thinking)
- Observability setup (standardising on LGTM stack with tiered alerting)
- The wall strategy itself (defining product boundaries)

**Already product (should be deepened, not rebuilt):**
- The Rust/Axum service architecture (repeatable, documented patterns)
- The PostgreSQL + Snowflake dual-store architecture (well-reasoned, benchmarked, documented)

**Already commodity (use, don't build):**
- PostgreSQL, Snowflake, Redis, NATS, S3, Kubernetes
- Python/Rust toolchains
- OpenTelemetry, Grafana ecosystem

The gap is in the transition zone. The bespoke work has been done. The commodity layer is well-chosen. The product layer is being designed (ingestion, walls, maturity dimensions) but not yet built and proven. That's where the leverage is — and it's exactly where the current approach becomes unsustainable.

The journey from bespoke to product isn't a technical project. It's a decision to stop building things once and start building things that last. The revenue trade-off is real. The customer pushback is real. The short-term slowdown is real. But the alternative — an impossible breadth of bespoke work — is also real, and it doesn't get better with time.

---

## Reading

- Simon Wardley, *Wardley Maps* (free on GitHub) — the evolution axis is this entire note in spatial form
- Geoffrey Moore, *Crossing the Chasm* — the specific challenges of moving a product from early adopters to mainstream market
- Marty Cagan, *Inspired* — product management discipline (what to build, how to validate, how to say no)
- Eric Evans, *Domain-Driven Design* — bounded contexts are product boundaries; the ubiquitous language is the configuration surface
- Stewart Brand, *How Buildings Learn* — how physical structures evolve through use; surprisingly applicable to software product evolution. The "shearing layers" concept (site → structure → skin → services → space plan → stuff) maps directly to commodity → product → bespoke layers in a tech stack
