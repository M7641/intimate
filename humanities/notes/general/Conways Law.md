# Conway's Law

"Any organization that designs a system will produce a design whose structure is a copy of the organization's communication structure." — Melvin Conway, 1967

This isn't a suggestion. It's closer to a law of physics for software. The architecture you ship will mirror the team that built it, regardless of what architecture you intended.

## Why This Explains the Current Situation

Consider the core problem: one person (or a very small team) managing ten or more complex bespoke projects, each with different structures, different customer requirements, different failure modes. The usual proposed solutions are "let them rot" or "build walls." But the deeper question is: why does the architecture demand this impossible breadth in the first place?

Conway's Law answers it. If a small team owns everything, the system becomes a monolith with tendrils — each customer project is structurally connected to one person's mental model. There's no separation because there's no organisational separation. The architecture can't have walls if the team doesn't have walls.

The inverse is also true (the "Inverse Conway Manoeuvre"): if you want a specific architecture, organise the team to match it. Want independent, wall-bounded products? You need independent, wall-bounded teams — each owning one product boundary (Quote→Order, Inventory, Merchandising), with clear interfaces between them.

## Team Topologies

Matthew Skelton and Manuel Pais formalised this in *Team Topologies* (2019). Four team types:

**Stream-aligned teams** — own a single flow of work (a product, a customer segment, a business domain). They ship independently. This is the target state for each product boundary.

**Enabling teams** — help stream-aligned teams adopt new capabilities. Temporary by design. Example: a team that helps customer teams adopt a new pipeline or set up observability, then moves on.

**Complicated-subsystem teams** — own something that requires deep specialist knowledge (ML model development, a Rust feature engine). Other teams consume their output through clean interfaces.

**Platform teams** — provide self-service internal products that reduce cognitive load on stream-aligned teams. The data-movement pipeline, the observability stack, the ingestion layer — these are platform capabilities. If they require specialist knowledge to use, the wall strategy fails.

The key insight: the current pain isn't a capacity problem, it's a topology problem. One person functioning as a stream-aligned team, an enabling team, a complicated-subsystem team, and a platform team simultaneously. No architecture survives that.

## Cognitive Load as the Limiting Factor

Team Topologies introduces three types of cognitive load:

- **Intrinsic** — the inherent complexity of the domain (pricing logic, inventory models)
- **Extraneous** — complexity from tooling, processes, unclear boundaries (debugging someone else's bespoke pipeline)
- **Germane** — the good kind: learning, improving, deepening expertise

The overloaded engineer is drowning in extraneous load. The wall strategy is an attempt to eliminate it — observe only edges, ignore internals. But walls only hold if someone else owns the internals. Without team boundaries, walls are just wishes.

## What This Means Practically

The product boundaries (Quote→Order, Inventory, Merchandising) are also *team* boundaries. Each one needs an owner, a clear interface contract (schema, API, event stream), and permission to make internal decisions independently. If one person still needs to understand all three, they aren't really separate.

This also explains why a single client's project can sit as a set of empty stubs. It's not lack of thinking — it's that the client requires thinking across too many domains simultaneously (pricing, bidding, quoting, audiences, catalogue, tiers, data movement, pipeline). Each stub is a potential product boundary that never got the dedicated attention to crystallise, because attention was spread across everything.

Conway's Law isn't a problem to solve. It's a constraint to design within. The question isn't "how do I build better walls in the code?" It's "how do I organise people so that walls emerge naturally?"

## Reading

- *Team Topologies* — Skelton & Pais (the practical framework)
- *Thinking in Systems* — Donella Meadows (systems dynamics, feedback loops, why organisations resist structural change)
- Conway's original paper: "How Do Committees Invent?" (1968) — short, sharp, still relevant
- *The Mythical Man-Month* — Brooks (the classic on why adding people doesn't solve structural problems)
