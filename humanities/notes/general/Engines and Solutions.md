## How to Mentally Define an Engine, and How to Build One

## The distinction, stated precisely

A *solution* answers a question once. "What price should this quote be?" You compute it, you ship it, the question is closed. The artefact you produce is an *answer*.

An *engine* answers a *class* of questions indefinitely — for inputs it has never seen, operated by people who did not build it, in conditions its authors did not anticipate. The artifact you produce is not an answer but a *machine that produces answers*. The difference is not size or sophistication. A 200-line script can be an engine; a 50,000-line system can be a glorified solution wearing a uniform. The difference is structural, and it is decided almost entirely by how you think *before* you write code.

This essay is about that thinking — the specific mental moves that turn a solution into an engine — and then about how those mental moves become an actual architecture. The claim is that engine-ness is not an emergent property you discover after the fact by refactoring. It is a stance you adopt at the start, a set of commitments about what is allowed to change and what is not. Get the stance right and the architecture mostly writes itself. Get it wrong and no amount of layering, dependency injection, or microservices will save you, because you will simply have built a large, distributed solution.

## Why this is worth getting right

The cost of the solution/engine confusion is asymmetric and it compounds. When you build a solution and later need an engine, you don't pay the difference once — you pay it every time the world changes. A new region wants different rounding rules. A new customer brings a different objective. A regulator imposes a constraint you never modeled. Each of these arrives as a fire because the original answer hard-coded an assumption that has now been violated, and the assumption is welded to logic that touches everything.

Conversely, over-engineering a one-off into a configurable, pluggable, abstract cathedral is its own waste — you pay an up-front tax in indirection and ceremony for flexibility nobody will ever use. So the first discipline is not "always build engines." It is to know, deliberately and early, *which one you are building* and *why*, and to be able to defend that choice. The rest of this essay assumes you have decided you need an engine. The interesting question is then: what does that decision actually commit you to?

## Part I — Mentally defining an engine

### Move 1: Find the verb, not the noun

The first mistake people make is to think about the engine in terms of what it *contains* — the models, the rules, the data sources, the integrations. That is thinking in nouns, and nouns are exactly the things that should be allowed to change. An engine is a *verb*. It *does* something to inputs to produce outputs, and that doing is a process that stays the same even as every noun around it is swapped out.

So the opening move is to articulate the engine as a single sentence with the shape: *given X, the engine repeatedly does Y to produce Z.* For quote pricing, the verb is: "generate candidate prices, predict the conversion probability of each, evaluate them against an objective, apply constraints, and return a bounded recommendation." Notice what that sentence does *not* mention: it doesn't say *which* model predicts conversion, *which* objective, *which* constraints, or *which* data store. Those are nouns. They are deliberately absent, and their absence is the whole point.

The test for whether you have found the real verb is this: try to change a noun and see if the verb survives. Swap LightGBM for a neural net — does "predict conversion at each candidate" still describe what happens? Yes. Change the objective from margin to volume — does "evaluate against an objective" still hold? Yes. If you can run through every noun you can imagine substituting and the sentence still reads true, you have found the invariant loop. That loop is the engine. Everything you had to *not* mention is the fuel.

A useful sharpening: the verb should be expressible without reference to your company, your customer, your current quarter, or your tech stack. If your one-sentence definition only makes sense for *your* deployment, you have described a solution. The engine's verb should sound almost embarrassingly general — "score candidates, optimize against an objective subject to constraints" — because generality is exactly what lets it serve a class of questions rather than one.

### Move 2: Separate the invariant from the variant, ruthlessly

This is the core conversion move, and it is worth being precise about *what* varies, because "variation" is not one thing. There are at least four distinct kinds, and they get pushed out of the engine in different ways:

**Data** varies fastest and most. The actual quote requests, the historical conversions, the live inventory levels, the current FX rates. Data flows *through* the engine; it is never baked in. The engine treats data as an argument, not a constant.

**Configuration** varies per deployment but is stable within one. Which rounding increment, which currency, which feature flags are on, what the price floor is. Config is the set of knobs you'd expect an operator — not an engineer — to turn. The presence of a human who is *not* a developer turning these knobs is the surest sign you have correctly externalized them.

**Policy** varies by jurisdiction, customer segment, or business rule. The constraint that a price may never undercut cost by more than X. The rule that government clients get a fixed discount. Policy is interesting because it is *declarative* — it describes what must be true of the output — and a well-built engine lets you add a policy without touching the loop that enforces policies.

**Behavior / strategy** varies by intent. *Which* model predicts conversion, *which* algorithm searches the candidate space, *which* objective function defines "good." This is the deepest kind of variation because it is executable — it is code that the engine runs but does not own. This is what plugins are for.

The mental move is to take every concrete thing in your problem and ask: *which bucket is this?* The answer determines where it lives. Anything that lands in one of these four buckets must be evicted from the engine's core and re-admitted only through a defined opening — an argument, a config value, a policy object, a plugin interface. What remains after all four buckets are emptied is the engine. It should feel surprisingly small. If your "engine" still has a `if customer == "Acme"` somewhere in it, you have not finished emptying the buckets.

There is a subtle trap here. Some things *look* invariant because they have never changed, not because they *cannot* change. "We always price in GBP." "We always optimize for margin." The discipline is to ask not "has this changed?" but "is there any world in which a legitimate operator of this engine would want this to be different?" If yes, it is fuel, even if it has been constant for the entire life of the project. Engines are defined against the space of *possible* inputs, not the history of *actual* ones. This is the single hardest mental adjustment, because every assumption you've never had to question feels like bedrock.

### Move 3: Define the engine by its contract, before its contents

Here is Ousterhout's deep-module idea applied with full force: a great module has a *simple interface* hiding a *complex implementation*. The engine's identity is its contract — the promise it makes to the outside world — and that contract should be the very first thing you write down, before you have any idea how you'll fulfill it.

The contract for the pricing engine is something like: *given a well-formed quote request and a pricing context, return a recommendation that is bounded, constraint-satisfying, and accompanied by an explanation — or a typed refusal explaining why no valid price exists.* That is the whole external truth of the engine. Everything else — the model, the optimizer, the warehouse — is implementation that the contract is specifically designed to *hide*.

Writing the contract first does something psychologically important: it forces you to commit to the *shape* of the promise while you are still ignorant of the mechanism, which is exactly when you are least tempted to let the mechanism leak. If you design the contract after building the internals, the internals will bleed into it — you'll expose a `model_version` field because it was convenient, a `warehouse_connection` because you had one lying around — and each leak converts a bit of engine back into solution. A contract designed in ignorance of internals is automatically clean. A contract reverse-engineered from internals is automatically dirty.

The contract has three parts worth being explicit about. The **input shape** — what a caller must provide and what they may omit. The **output shape** — what they are guaranteed to receive, *including the failure cases*, which an engine treats as first-class outputs rather than exceptions to handle elsewhere. And the **invariants** — the promises that hold regardless of inputs: "the returned price is always within the configured bounds," "the explanation always references the constraints that bound it," "the same input with the same fuel always produces the same output." These invariants are the engine's reputation. They are what let people who did not build it *trust* it.

```
# The contract, written before any implementation exists.

QuoteRequest:
  customer_ref:   CustomerRef        # opaque to the engine
  line_items:     List[LineItem]
  context_ref:    PricingContextRef  # which fuel-set to use
  requested_at:   Timestamp

Recommendation:
  status:         "PRICED" | "NO_VALID_PRICE"
  price:          Money?              # present iff PRICED
  bounds:         Interval            # the floor/ceiling that applied
  binding_constraints: List[ConstraintRef]  # what shaped this answer
  objective_value:     Float?
  explanation:    Explanation         # always present, even on failure
  fuel_fingerprint: Hash              # which models/config/policies ran

# The promise:
#   price(request, fuel) -> Recommendation
#
# Invariants the engine guarantees for ALL inputs:
#   1. status == PRICED  =>  bounds.lo <= price <= bounds.hi
#   2. determinism: price(r, f) == price(r, f) always
#   3. explanation references every binding constraint
#   4. no exception escapes; infeasibility is a NO_VALID_PRICE result
```

Notice that the contract mentions `fuel_fingerprint` but not *what the fuel is*. The engine commits to telling you *which* fuel ran (so results are reproducible and auditable) without committing to *what kinds* of fuel exist. That is the seam between engine and fuel made explicit in the interface itself.

### Move 4: Draw the membrane — decide what is inside

Every engine has a membrane: a boundary across which fuel enters and answers leave, and *nothing else crosses*. Mentally defining the engine is largely the act of drawing this membrane and then being honest about everything that wants to sneak across it.

Inside the membrane lives the invariant loop and nothing else: the orchestration of "generate, score, evaluate, constrain, return," plus the machinery that enforces the contract's invariants. Outside the membrane lives all four kinds of variation — data, config, policy, behavior — and crucially, everything the engine *depends on but does not control*: the database, the network, the clock, the model server, the message bus. These are the things that fail, that are slow, that differ between dev and prod, that you want to fake in a test. The engine must reach them only through holes you deliberately cut in the membrane, never by reaching out directly.

The discipline of the membrane is what makes an engine testable, portable, and trustworthy. If the loop talks straight to PostgreSQL, you cannot run it without PostgreSQL, you cannot run two of it against different stores, and you cannot reason about it without reasoning about PostgreSQL too. If instead it talks to a `PriceHistoryPort` — an abstract opening in the membrane — then PostgreSQL is just one thing you can plug into that opening, swappable for an in-memory fake, a different warehouse, or a recorded fixture. The engine doesn't know or care. That not-knowing is its strength.

So the final mental move is to inventory every dependency and ask, for each: *does the engine control this, or merely use it?* Everything it merely uses gets a port — a hole in the membrane with a defined shape — and the actual thing becomes an *adapter* that fits that hole from the outside. The engine is then defined as precisely the code that lives entirely inside the membrane and speaks to the outside only through ports.

## Part II — From mental model to architecture

The four moves above are a way of *thinking*. Now they become structure. The architecture of an engine is just the four moves made concrete and enforced by the type system, the module boundaries, and the build graph — so that the next person literally cannot violate them without the compiler or the linter complaining.

### The spine: contract at the center, dependencies inverted

The architectural expression of "contract before contents" and "draw the membrane" is the *dependency inversion principle*, and it gives you the now-classic hexagonal / ports-and-adapters shape. The rule is brutally simple: **the engine core depends on nothing concrete. Everything concrete depends on the engine core.** Arrows point inward. The database adapter imports the engine's `PriceHistoryPort` interface; the engine never imports the database. The model-server client implements the engine's `ConversionModel` interface; the engine never imports the model server.

```
                 ┌─────────────────────────────────────┐
   adapters →    │              ENGINE CORE             │   ← adapters
                 │                                      │
  HTTP handler ──┼─▶ price(request, fuel): Recommendation│
                 │      │                               │
                 │      ▼  (the invariant loop)         │
                 │   generate → score → evaluate →      │
                 │            constrain → bound          │
                 │      │        │         │             │
                 │   [Port]   [Port]   [Port]            │
                 └──────┼────────┼─────────┼────────────┘
                        ▼        ▼         ▼
                 CandidateGen  ConversionModel  PolicySet   ← fuel, plugged in
                 (strategy)     (behavior)      (policy)
                        ▲        ▲         ▲
                  these IMPLEMENT the engine's interfaces;
                  the engine never imports them.
```

The single most consequential architectural decision you will make is the direction of these arrows. If the engine package can `import` a concrete model, a concrete database, or a concrete customer rule, then no naming discipline will save you — the engine is structurally a solution, because it cannot exist without those specific things. If the engine package imports only its own interfaces and standard types, it is structurally an engine, because it can be compiled, tested, and reasoned about with *zero* concrete fuel present. A good litmus test: can you build and unit-test the engine with every adapter replaced by a fake, in memory, with no network? If yes, the arrows point the right way.

### Ports: the shape of every hole in the membrane

A port is an interface that expresses *what the engine needs* in the engine's own vocabulary — never in the vocabulary of the thing that will satisfy it. This is the discipline that keeps adapters from leaking. The engine doesn't need "a Postgres connection"; it needs "the ability to fetch conversion history for a customer." Name the port after the *need*, not the *provider*.

```
# Ports are defined INSIDE the engine, in the engine's language.

interface ConversionModel:               # behavior / strategy plug
    predict(candidate: Candidate, ctx: PricingContext) -> Probability

interface CandidateGenerator:            # behavior / strategy plug
    candidates(request: QuoteRequest, ctx: PricingContext) -> Iterable[Candidate]

interface Objective:                     # behavior plug
    score(candidate: Candidate, p: Probability, ctx: PricingContext) -> Float

interface PriceHistoryPort:              # data plug
    history(customer: CustomerRef, window: Duration) -> List[PricePoint]

interface Constraint:                    # policy plug (declarative)
    name: ConstraintRef
    permits(candidate: Candidate, ctx: PricingContext) -> bool
    # NB: a Constraint answers a yes/no question. It does NOT
    # know about pricing strategy. It only declares what is allowed.

interface Clock:                         # ambient dependency, made explicit
    now() -> Timestamp
```

Two things make these ports good. First, every method speaks in the engine's domain types (`Candidate`, `PricingContext`, `Probability`) — not in JSON, SQL rows, or HTTP responses. Translation between the outside world's formats and the engine's types happens in the *adapter*, never in the engine. Second, the ports are narrow: each expresses one need. A fat port that bundles fetching, predicting, and logging into one interface forces every adapter to implement all of it and re-couples concerns the engine worked hard to separate. Narrow ports keep adapters honest and substitution cheap.

### The fuel taxonomy, made concrete

Recall the four kinds of variation. Each gets a distinct architectural treatment, and conflating them is a common source of mess:

**Data** enters as method *arguments* through data ports (`PriceHistoryPort` above). It is never stored in the engine; it is requested when needed and flows through.

**Configuration** enters as an immutable value object constructed at the membrane and passed into the loop. The engine reads it but never mutates it, and critically, never *acquires* it — config is handed to the engine, not fetched by it.

```
PricingConfig:                     # immutable, validated at the edge
    currency:        Currency
    rounding:        RoundingRule
    global_floor:    Money
    global_ceiling:  Money
    candidate_budget: int          # how many candidates to generate
    # ...knobs an OPERATOR turns, not an engineer
```

**Policy** enters as a *set* of `Constraint` objects. The architectural payoff: adding a new policy means writing a new `Constraint` and adding it to the set — it does *not* mean editing the loop that applies constraints. The loop iterates over an open-ended collection; the collection is fuel.

**Behavior** enters as plugged-in implementations of the strategy ports (`ConversionModel`, `CandidateGenerator`, `Objective`). Swapping LightGBM for a neural net is "construct a different `ConversionModel` adapter and pass it in." The engine never changes.

All four are bundled into a single `Fuel` (or `PricingContext`) object that is assembled *outside* the engine and handed across the membrane. The engine's signature — `price(request, fuel)` — makes the separation visible in the type system: requests are the data-of-the-moment, fuel is everything-that-varies-by-deployment. The engine itself appears in neither; it is the function.

```
Fuel:                              # assembled at the edge, passed inward
    config:      PricingConfig
    model:       ConversionModel
    generator:   CandidateGenerator
    objective:   Objective
    constraints: List[Constraint]
    history:     PriceHistoryPort
    clock:       Clock
    fingerprint: Hash              # identity of this exact fuel-set
```

### The core: the invariant loop, and nothing else

With ports and fuel defined, the engine core becomes shockingly small — which is the goal. It is the verb from Move 1, transcribed:

```
function price(request: QuoteRequest, fuel: Fuel) -> Recommendation:

    validate(request)                                     # contract input guard

    candidates = fuel.generator.candidates(request, fuel) # behavior plug
    if candidates.is_empty():
        return no_valid_price(reason="no candidates", fuel)

    scored = []
    for c in candidates:
        p   = fuel.model.predict(c, fuel)                 # behavior plug
        val = fuel.objective.score(c, p, fuel)            # behavior plug
        if all(k.permits(c, fuel) for k in fuel.constraints):  # policy plug
            scored.append((c, val, p))

    if scored.is_empty():
        return no_valid_price(reason="all candidates infeasible", fuel)

    best = argmax(scored, by=val)
    bounded = clamp(best.candidate.price,                 # invariant enforcement
                    fuel.config.global_floor,
                    fuel.config.global_ceiling)

    return Recommendation(
        status               = "PRICED",
        price                = bounded,
        bounds               = Interval(fuel.config.global_floor,
                                        fuel.config.global_ceiling),
        binding_constraints  = constraints_that_bound(best, fuel),
        objective_value      = best.val,
        explanation          = explain(best, fuel),
        fuel_fingerprint     = fuel.fingerprint,
    )
```

Read what this loop *is* and *isn't*. It is the generate–score–evaluate–constrain–bound process and the enforcement of the contract's invariants (validation in, clamping to bounds, never throwing, always explaining). It is *not* any model, any database, any customer rule, any currency, any objective. Every one of those is reached through `fuel`. You could keep this loop unchanged for a decade while replacing every model three times, adding forty constraints, and supporting nineteen currencies. *That* is what makes it an engine: the rate of change of the core approaches zero while the rate of change of the fuel is unbounded.

### The composition root: where solutions are assembled

If the engine knows no concrete fuel, *something* must. That something is the **composition root** — a thin layer at the very edge of the system, outside the membrane, whose only job is to assemble fuel and adapters and hand them to the engine. This is where your actual deployment's specifics live, and it is the *only* place they are allowed to live.

```
# composition_root.py  — the ONLY place that knows concrete things.
# This file is allowed to be "a solution." It is wiring, not logic.

function build_uk_margin_engine(env) -> (QuoteRequest -> Recommendation):
    config = PricingConfig(
        currency = GBP, rounding = NearestPenny,
        global_floor = Money(GBP, 0.01), global_ceiling = Money(GBP, 1_000_000),
        candidate_budget = 256,
    )
    model      = LightGBMConversionModel(load("uk_conv_v7.bin"))   # concrete!
    generator  = GridCandidateGenerator(step = 0.5)
    objective  = MarginObjective(cost_lookup = env.cost_service)
    constraints = [
        FloorConstraint(config.global_floor),
        NeverUndercutCostBy(max_pct = 0.15),
        GovClientFixedDiscount(pct = 0.10),                        # a UK policy
    ]
    history    = PostgresPriceHistory(env.db)                      # concrete!
    fuel = Fuel(config, model, generator, objective,
                constraints, history, SystemClock(),
                fingerprint = hash_of(everything_above))

    return (request) => engine.price(request, fuel)               # partial application
```

This is the crucial architectural insight that ties the whole thing together: **an engine plus a fully-specified fuel-set is, once again, a solution.** "The UK margin-pricing service" is a solution — a specific answer-producer for a specific deployment. But it is now a *thin* solution, a few dozen lines of wiring, and you can stand up "the German volume-pricing service" right beside it by writing another composition root, reusing the identical engine. You have not eliminated solutions; you have *isolated* them to the edge and made them cheap. The engine is the reusable verb; each composition root is a sentence built from it.

This also resolves the apparent paradox of "don't build solutions." You always ship a solution — users pay for answers, not for verbs. The engineering achievement is to make the solution a thin shell around an engine, so that the next solution costs a composition root instead of a rewrite.

### Three properties an engine must treat as first-class

Beyond the loop and the ports, an engine — *because* it will be operated by people who did not build it, on inputs no one foresaw — has to make three things architectural, not afterthoughts:

**Determinism and reproducibility.** Given the same request and the same fuel fingerprint, the engine must produce the same answer. This is why the clock is a port, why randomness (if any) takes an explicit seed from fuel, and why the fuel fingerprint is part of the output. Without this, you cannot debug a complaint ("why did this quote get £900?") because you cannot reproduce the conditions. Determinism is not a nicety; it is what makes the engine *auditable*, and auditability is what lets strangers trust it.

**Observability as output, not logging.** The engine returns *why* — the binding constraints, the objective value, the explanation — as part of the contract, not as a side-channel log. An engine that can only say *what* it decided is half-built; one operated by non-authors must say *why*, in structured form, every time. Treat the explanation as a first-class output with the same rigor as the price.

**Typed failure.** Infeasibility — "no valid price exists under these constraints" — is a normal, expected output, not an exception to be caught somewhere upstream. An engine that throws on the hard cases pushes its hardest reasoning onto its callers, which is precisely the coupling it was supposed to absorb. The contract's `NO_VALID_PRICE` status is the engine taking responsibility for its own edges.

## Part III — How engines decay, and how to notice

Engines do not usually die by redesign; they die by a thousand small leaks, each individually reasonable. Knowing the failure modes is how you keep the membrane intact over years.

**The interface leak.** Someone exposes `model_version` or `warehouse_url` on the contract "just for this one integration." Now every caller can see, and will eventually depend on, an internal detail. The moment a caller relies on something you wanted to keep swappable, you can no longer swap it without breaking them — the engine has silently become a solution with a published internal structure. *Detection:* review every field on the public contract and ask "would I be free to delete the thing behind this tomorrow?" If not, it shouldn't be on the interface.

**The special case.** `if customer == "Acme"` appears in the loop. It is fast, it ships, and it is the beginning of the end, because the next special case follows, and soon the loop encodes a dozen customers and is no longer invariant. *Detection:* any conditional in the core that names a concrete value from outside the engine is a leak; it belongs in a `Constraint`, a config flag, or a strategy plugin.

**The reach-out.** The core, needing some datum, imports a client and fetches it directly rather than going through a port. The membrane is punctured; the engine can no longer be tested or run in isolation. *Detection:* the engine package's import list. If it imports anything concrete and external, the arrow points the wrong way.

**The fat port.** A port grows a tenth method because one adapter happened to need it, forcing all adapters to implement what only one uses. Substitution gets expensive, and the temptation to "just use the concrete thing" returns. *Detection:* ports that no longer fit on a screen, or methods only one adapter implements meaningfully.

**The frozen assumption.** Not a code leak but a thinking leak: a "constant" that should have been fuel. It surfaces the day the assumption is violated and you discover it was load-bearing in forty places. *Detection:* periodically re-run Move 2's question — "is there a world where a legitimate operator wants this different?" — against things you currently treat as invariant.

## A working discipline

If you want a checklist to carry into the next design, it is just the moves, made operational:

Write the verb in one sentence; confirm it survives swapping every noun. Write the contract — input, output including failures, invariants — *before* any implementation. Sort every concrete thing into data, config, policy, or behavior, and give each its architectural home outside the core. Draw the membrane; give every external dependency a port named after the *need*, not the provider; point all arrows inward. Keep the core to the loop plus invariant-enforcement and prove it by unit-testing it with every adapter faked. Push all concreteness into composition roots at the edge, where solutions are allowed to live and stay thin. Make determinism, structured "why," and typed failure first-class. Then defend the membrane forever against the five leaks.

The deepest idea underneath all of it is a shift in what you consider your deliverable. A solution-builder's deliverable is the answer. An engine-builder's deliverable is the *generator* of answers — and the test of whether you've built one is not whether it answers today's question well, but whether someone who has never met you can feed it a question you never imagined, trust the answer, and understand why it came out that way. Build for that stranger, and you have built an engine. Build for today's question, and however large it grows, you have built a solution.
