A maturity model can name the dimensions and boundaries of a system but still lack a spatial framework — a way to see where things are, where they're going, and what to do about it. Wardley Mapping provides that.

## The Core Model

A Wardley Map has two axes:

**Vertical: Value Chain** — the chain of dependencies from user need (top) down to underlying components (bottom). A user needs pricing recommendations (top), which needs a model (middle), which needs features (lower), which needs clean data (bottom). Everything above depends on everything below.

**Horizontal: Evolution** — every component moves left to right over time through four stages:

1. **Genesis** — novel, uncertain, requires exploration. (A new ML approach for a customer, a prototype agentic pipeline)
2. **Custom-built** — understood but still bespoke. (Current customer projects — each one hand-crafted for the specific client)
3. **Product** — standardised, repeatable, configurable. (Defined boundaries and maturity dimensions, moving toward a reusable offering)
4. **Commodity/Utility** — invisible infrastructure, pay-per-use. (PostgreSQL, Snowflake, S3, Kubernetes)

The key insight: **components at different evolutionary stages need different management approaches**. Genesis needs exploration and tolerance for failure. Custom needs skilled engineers. Product needs productisation discipline. Commodity needs operational efficiency. Treating them all the same — which is what happens when one team owns everything — guarantees poor outcomes.

## Mapping the Current Landscape

Plotting the current landscape against these axes:

**Commodity (right side — use, don't build):**
PostgreSQL, Snowflake, Redis, NATS, Kubernetes, Grafana, S3. These are settled. They're infrastructure choices, not innovation areas.

**Product (moving right — standardise):**
A data ingestion pipeline, observability stack (LGTM), schema contracts, the parsing-not-validation layer. These are understood well enough to productise. The underlying data-movement patterns should be repeatable across customers without reinvention.

**Custom-built (middle — where the pain is):**
Customer-specific pipelines (one per client), pricing models, bespoke data transformations. These are components stuck in custom-built that should be evolving toward product. Each customer gets a hand-built version of something that's structurally similar.

**Genesis (left side — explore carefully):**
Agentic database architecture, ML-driven decision systems, a Rust feature engine serving models. These are genuinely novel and warrant experimentation, but they shouldn't be mixed with production customer commitments.

## The Strategic Implications

**Stop customising commodities.** If you're spending time configuring PostgreSQL or Snowflake differently for each customer, that's waste. Standardise infrastructure.

**Productise the custom-built middle.** The ingestion pipeline, the data movement patterns, the validation layers — these are mature enough to be products with configuration, not projects with code. Every hour spent building a bespoke version for one customer is an hour not spent making the reusable version better.

**Protect genesis from production pressure.** Agentic architecture and the Rust/ML serving stack are genuinely new. They need space to evolve without being forced into customer timelines. If they get dragged into "we need this by June," they'll become more bespoke, not more general.

**Use evolution to say no.** When a customer asks for something that would push a component *backwards* on the evolution axis (from product toward custom), that's a signal. The map makes this visible. "We can do X because it's configuration of our product. We can't do Y because it would require custom development that moves us backward."

## How to Actually Make a Map

1. Start with a user need at the top (e.g., "customer gets accurate pricing recommendations")
2. List every component required to deliver it, as a dependency chain going downward
3. Place each component on the evolution axis based on how mature/standardised it currently is
4. Draw the dependencies (lines between components)
5. Mark movement — arrows showing where you want components to evolve

Do this for one customer with enough detail to see the full value chain. Then overlay a second customer. The overlapping components in the custom-built zone are your productisation targets.

## Reading

- Simon Wardley's book is free: *Wardley Maps* (available on GitHub as a series of blog posts turned into chapters)
- LearnWardleyMapping.com — interactive tutorials
- Ben Mosior's Wardley Mapping Canvas — practical workshop tool
- The connection to Team Topologies is explicit: Skelton & Pais reference Wardley Maps as the way to decide team boundaries
