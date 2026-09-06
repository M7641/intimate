
*The purpose here is to turn instinct into a framework: a set of named concepts, tests, and trade-off axes that let you evaluate any infrastructure proposal for a multi-tenant ML SaaS — from principles rather than from gut.*

---

## 1. How to use this framework

A proposal of this kind tangles three different kinds of claims together: claims about physics (the data can't move), claims about design (where services should live), and claims about people (the team is weak and the mandate is toxic). Each is arguable, but they must be argued separately, because they have different standards of proof and different audiences. This framework deals with the first two rigorously and returns to the third only at the end.

When a new proposal lands, the working method is:

1. Decompose it into capabilities (Section 6 gives the ML-specific capability list).
2. Assign each capability to a plane using the boundary tests (Section 3).
3. Score the proposal on the trade-off axes (Section 5).
4. Write down falsifiable predictions before the thing ships (Section 11).
5. Only then, separately, form a view on execution quality and mandate (Sections 12–13).

---

## 2. The core frame: control plane vs data plane

Every capability in the system belongs to one of two planes:

**The data plane** is everything that touches customer data at volume or serves customer-facing work: the warehouse, S3, feature pipelines, training jobs, batch scoring, online inference, the applications rendering data back to the customer. It is where the actual value is produced, where the heavy compute burns, and where the security boundary must hold.

**The control plane** is everything that decides, defines, and records: workflow definitions; orchestration and scheduling; the model registry; tenant metadata; auth and identity; versioning; quota and billing; and deployment configuration. It touches *metadata about* data and work, never the data itself.

This is not an invented distinction — it is the load-bearing idea in most serious infrastructure:

- **Databricks (classic architecture):** control plane in Databricks' account (web app, job scheduler, notebook service, cluster manager); compute plane in the *customer's* cloud account, next to the customer's data. The data never has to enter Databricks' side.
- **AWS's "static stability" doctrine:** data planes are deliberately built to keep functioning when their control plane fails. EC2 instances keep running when the EC2 API is down; Route 53's DNS resolution (data plane) is engineered to survive outages of the console and API (control plane). The planes have different availability targets *on purpose* — the data plane is engineered to a higher standard, and the control plane is kept out of the request path.
- **Kubernetes:** the API server and controllers hold *desired state*; kubelets on every node do the actual work and keep containers running even if the control plane goes away. The reconciliation model — declare centrally, execute locally — is directly reusable for this problem (Section 7).

A thin central coordination service is a control plane. The common observation that such a service "does not really do much, it just defines and structures things… it doesn't actually do anything" is not a criticism of a control plane — it's the *definition* of one. A control plane that "does a lot" is a badly-scoped control plane. So the real debate is never "central service: yes or no." It is: **is the boundary between the planes drawn in the right place, and is the control plane competently built?**

The usual framing — one CPU running one service, or the same service on many CPUs localised to the tenant — is therefore a false binary. The mature answer is *both, split by plane*: a thin central control plane, and a fat, replicated, tenant-local data plane. The instinct for tenant-local is right for the data plane. The "organisational layer" that skeptics concede is right for the control plane. Most disagreement with a central-services push is actually a disagreement about *specific capabilities being placed in the wrong plane* — which is a much stronger, more precise argument than being against centralisation.

---

## 3. Boundary tests

Three tests resolve most placement arguments. Apply them to any capability someone wants to centralise.

**Test 1 — The outage test.** *If the central service is down for an hour, what stops?* The correct answer: no new work can be defined, scheduled, or promoted — but running jobs finish, served models keep serving, applications keep reading. If a control-plane outage halts tenant work in flight, the control plane has absorbed data-plane responsibilities. Corollary: the control plane must never sit in the synchronous request path of the data plane. A tenant service may *consult* the control plane (fetch config, resolve a model version) but must cache the answer and degrade gracefully.

**Test 2 — The data-touch test.** *Does customer data, at any volume, flow through the central service?* If yes, you have moved the security boundary, the compliance story, and the blast radius — usually without anyone pricing that in. Metadata, aggregates, and artefact *references* may flow centrally; rows may not. This is why a "central registry that loads model artefacts and runs inference on small blobs" idea should make you uneasy: the moment small blobs of tenant data transit a shared service, "the data never leaves the tenant" stops being true, and that sentence is probably load-bearing in the contracts. Central *registry*, tenant-local *serving* is the clean cut. (There is a legitimate carve-out — Section 6.5 — but it must be an explicit, contractual decision, not an architectural accident.)

**Test 3 — The credential test.** *Where do data-plane credentials live and travel?* The strongest technical objection to many central designs is warehouse credentials arriving in request headers to a central API. That makes the central service a **credential-forwarding hub**: it transitively holds live access to every customer's warehouse. Compromise it once — one dependency CVE, one logging misconfiguration that captures headers, one SSRF — and the blast radius is *every tenant simultaneously*. Compare the alternative: tenant-local workers that assume identity locally (IAM roles, workload identity federation, short-lived vault-issued credentials scoped to their own tenant). In that topology there is no single place where all credentials converge; the worst compromise is one tenant. A central service can be acceptable here only if it never *sees* long-lived credentials — e.g. it passes an opaque job spec to a tenant-local executor which resolves its own identity. Header-forwarding fails this test outright.

A fourth, softer test worth keeping in your pocket:

**Test 4 — The uniqueness test.** *Is this capability worth more because there is exactly one of it?* Workflow definitions, API contracts, the model registry, tenant metadata: yes — one source of truth is the entire point. Compute, connections, data processing: no — a second copy of compute is just more compute. Centralise the things that gain value from uniqueness; replicate the things that don't.

---

## 4. The physics: data gravity

The strongest foundation of the whole argument deserves to be stated as a law rather than an opinion: **compute moves to data, not the other way around.** The reasons stack:

- **Volume:** tenant warehouses are too large to consolidate, and egress/transfer costs punish movement even when it's technically possible.
- **Security and contract:** "your data stays in your tenant" is a sales-floor promise and probably a contractual one. Architecture that quietly violates it is a legal problem wearing a technical costume.
- **Blast radius:** pooled data means pooled failure and pooled breach. Siloed data means a bad day for one customer, not all of them.
- **Warehouse mechanics:** MPP warehouses (Redshift-style) are their own gravity well — the compute you'd want is already attached to the data, and dragging rows out to process them elsewhere is strictly worse than pushing the query down.

Consequence: training, feature engineering, batch scoring, and anything that reads rows at volume are **not negotiable** — they live in the tenant. Nobody serious contests this, and well-formed central-service designs don't contest it either: data, models, and artefacts all remain tenant-side. The entire argument is about the thin layer above.

The useful discipline: when someone proposes centralising something, first ask *"does this proposal actually move data, or only coordination?"* If only coordination, data gravity is irrelevant to the debate and you should not spend your credibility invoking it. Save the physics argument for when someone genuinely proposes moving rows.

---

## 5. The trade-off axes

These are the axes on which any placement decision should be scored. No design wins on all of them; the honest move is to name which axes a design sacrifices and check that the sacrifice is deliberate.

### 5.1 Blast radius and failure correlation

Centralised: one bug, one bad deploy, one outage hits every tenant at once, and the failures are perfectly correlated — the worst possible shape for an SLA and for a status page. Tenant-local: failures are isolated and uncorrelated, but there are N deployments each capable of failing in its own creative way, and the *aggregate* incident rate can be higher even while each incident is smaller. The mature question is not "which has fewer failures" but "which failure shape can we survive?" A SaaS business generally survives many small uncorrelated failures far better than rare total ones — churn is driven by "everyone was down Tuesday," not "tenant 14 had a bad hour."

### 5.2 Fleet management and the cost of change

This is the axis tenant-local advocates underweight, and it is the strongest *honest* argument for the central service — so own it before opponents raise it. A central service is cheap to *run*, but its real virtue is being cheap to *change*: one deploy, one migration, one place to patch a CVE, one version in production. The true cost of "the same service across many CPUs localised to the tenant" is not compute — it's operations: rolling an upgrade across N deployments; version skew when tenant 14 is three releases behind because its window was missed; debugging N slightly-divergent environments; the slow drift of per-tenant snowflakes as hotfixes accumulate. Per-tenant architectures that lack industrial-grade fleet management degenerate into a museum of versions. If you argue for tenant-local, you must arrive with the fleet-management answer already in hand (Section 7) — otherwise the centralisers win this axis by default, and it's a big axis.

### 5.3 Security topology

Covered by Test 3, but as a scoring axis: count the places where cross-tenant access converges. Every convergence point (a credential-forwarding API, a shared service role that can assume into any tenant, a pooled secrets store) is a single point whose compromise is total. The gold standard is that **no single component, if fully compromised, yields access to more than one tenant's data.** Central control planes can meet this standard — but only if they traffic in job specs and references, never in credentials or rows. Score any proposal by its worst-case single-compromise blast radius, not by its intended behaviour.

### 5.4 Connection and resource economics

The connection-economics point is technically sharp and worth formalising, because it recurs. A central service talking to N warehouses has two bad options: **hold** connections (N pools of live connection objects in memory — a resource liability, a failover nightmare, and an awkward fit with FastAPI's async loop, hence the thread-pool sprawl these designs exhibit), or **churn** them (open/validate/close per request — which hammers exactly the wrong component). On Redshift-style MPP warehouses, the leader node single-handedly does parsing, planning, catalog/metadata queries, and session management; a batch job that opens sessions and runs many small validation statements ("does this table exist," "check these columns") concentrates load precisely there, throttling the whole warehouse — including the customer's own queries. Postgres shrugs this off; a leader-node architecture does not. Mitigations if centralisation is forced: per-tenant connection pooling that lives *with* the tenant, batched validation (one catalog query, not thirty), and cached schema metadata with TTLs. But notice the deeper point: the connection problem is *created by* the central placement. A tenant-local worker has one warehouse, one small pool, no churn. Some problems are best solved by not creating them.

### 5.5 Consistency vs autonomy

The centralisers' real prize is one definition of workflow steps, one API contract, one way of doing things — and it is a real prize: onboarding a new tenant, reasoning about the system, and supporting customers all get easier when there is one shape. The cost is that every team now ships at the speed of the central team's release cycle and within the ceiling of the central team's imagination. Score a proposal by asking: *does the central layer define contracts and let teams implement freely beneath them, or does it define implementations?* Contracts centralise well. Implementations centralise badly. A central service that says "a workflow step has this schema, this lifecycle, these events" is a paved road; one that says "and you may only compute what our executor supports" is a ceiling.

### 5.6 Cost profile and scale-to-zero

It is worth keeping crisp: a coordination-only central service barely changes total system cost, because the expensive work still happens tenant-side — if anything, total cost rises slightly with the new hop. The corollary cuts both ways, though: the cost argument *for* centralisation is equally weak. Where cost genuinely differs is in the data plane's idle behaviour: per-tenant services that scale to zero (or near it) between jobs get you to a similar cost profile as pooling — with the caveat that cold starts interact badly with connection setup against warehouses, so "scale to zero" should mean scaling *workers*, not necessarily the component that holds the warehouse session. Cost is usually the least decisive axis; don't let anyone win the argument on it, in either direction.

### 5.7 Latency and the request path

Anything on the synchronous path of a customer-facing request must be fast and highly available — which argues for tenant-local placement of serving and app backends, and for keeping the control plane strictly *off* that path (Test 1). Batch and training work is latency-tolerant, so coordination hops cost nothing there. Score proposals by what they put on the hot path: a control-plane call per inference request is a design smell; a control-plane call per *deployment* is fine.

### 5.8 Operational load — who gets paged

Centralised: the platform team gets paged, and every incident is a cross-tenant incident with commensurate stakes. Tenant-local: pages shard across deployments; individual incidents are small but the on-call surface is wide, and without good fleet observability you get N dashboards instead of one. The honest question: *which team do you trust to hold a pager for a system whose failure affects every customer at once?* Note that this axis is where architecture and organisation legitimately meet — it is fair to weigh team capability when deciding how much blast radius to concentrate under one team's pager. That is not politics; it is engineering with honest inputs.

### 5.9 Observability and debuggability

A central service gives you one log stream, one trace context, one place to look — genuinely valuable at 3am. Distributed tenant-local execution demands deliberate investment: centralised log/metric aggregation (telemetry is metadata — it *should* flow centrally; this passes the data-touch test provided you police PII in logs), consistent trace IDs across the control/data boundary, and a fleet dashboard answering "what version is every tenant on, and what's failing where?" This axis genuinely favours centralisation *unless* the tenant-local side builds aggregation properly — so put it in your design rather than conceding the axis.

---

## 6. Mapping the ML lifecycle onto the planes

The generic frame, applied to each ML capability. This is the decomposition to run any proposal through.

### 6.1 Training

Tenant-local, non-negotiably: it reads the full dataset (gravity), burns the heaviest compute, and produces artefacts that are representations of that tenant's data. What centralises is everything *about* training: the definition of the training workflow, hyperparameter/config templates, the code (one repo, versioned, shipped to tenants — not N forks), and the record of what ran where with what result. A common symptom — "nobody can find anything that says how models are actually trained" — is a control-plane documentation failure, and exactly the thing a competent control plane would fix: with a real registry and workflow definition, "how are models trained" has a canonical, inspectable answer. That observation argues that the *concept* is sound and the *execution* is missing.

### 6.2 Feature engineering and ingestion

Tenant-local execution, same physics. The central layer legitimately owns feature *definitions* and ingestion *contracts* — schemas, expectations, validation rules — because those gain value from uniqueness (Test 4). A common ingestion-API grievance fits here: the failure is not that ingestion contracts were centralised, it's that the implementation was poor and mandatory simultaneously (Section 12).

### 6.3 Model registry and artefact storage

The registry is the purest control-plane capability in ML infrastructure: names, versions, lineage (data snapshot, code version, config), metrics, promotion state, and *pointers* to artefacts. The artefact bytes themselves stay in tenant S3 — the registry stores the reference and the hash, not the object. This gets you the single source of truth ("what is live for tenant X?") with zero data movement. Promotion becomes a control-plane state change ("v12 → production for tenant X") that tenant-local serving observes and acts on. If you build one central thing well, build this.

### 6.4 Batch scoring

Tenant-local, same reasoning as training: full-volume reads, output written back to the tenant warehouse. Centralise the schedule and the definition; execute locally. This is also where the connection-churn prediction (Section 5.4, Section 11) will first come due — batch jobs that validate inputs per-statement against the leader node.

### 6.5 Online inference

Default: tenant-local serving that loads artefacts from tenant S3, guided by the central registry's promotion state. The carve-out sometimes proposed — a central service running inference on small payloads — is legitimate *only* as an explicit product decision: some deployments may be too small to justify resident serving infrastructure, and pooled inference may be the only economic option for them. But it must be priced honestly: the moment tenant rows transit shared compute, the data-residency story changes, per-tenant model isolation must be actively enforced in a shared process, and noisy-neighbour latency appears. It's a *tiering* decision (small tenants pooled, large tenants siloed — the bridge model, Section 7), never a silent architectural default.

### 6.6 Monitoring, drift, and evaluation

The nuanced one. Drift *computation* happens tenant-side (it reads the data). Drift *metrics* — aggregates, distributions, scores — flow centrally, and should: cross-fleet questions ("is v12 degrading everywhere or just for tenant X?") are exactly what a control plane is for, and aggregates pass the data-touch test. Two cautions: high-cardinality aggregates and raw feature-value examples can quietly reconstruct customer data (a "top 20 values of this column" stat is data, not metadata), and alerting must not depend on the control plane being up (Test 1 — evaluate centrally, but page from somewhere that survives).

### 6.7 Retraining triggers and workflow orchestration

The scheduler/orchestrator is control-plane by nature — it decides *when* and *what*, not *how*. The clean pattern: the central orchestrator emits job specs (tenant, workflow, version, config — no credentials, no data); a tenant-local executor picks them up, resolves its own identity, runs the steps, reports status back. This is the "pass a job to a workflow-like entity" shape, and it's the right one. The quarrel, when there is one, is rarely with this design; it's with who's building it and what they mandate around it.

---

## 7. The architecture spectrum, and the pattern that resolves it

The standard SaaS tenancy vocabulary (worth using in meetings — shared vocabulary moves arguments faster):

- **Silo:** every tenant gets a full stack. Maximum isolation, maximum fleet cost. This is roughly what a data plane under strict data-residency constraints looks like, by necessity.
- **Pool:** all tenants share one stack, separated logically. Maximum efficiency, maximum blast radius, and unavailable for data (gravity, contracts) — only ever an option for coordination and, possibly, small-tenant serving.
- **Bridge:** deliberate hybrid — pooled control plane, siloed data plane, possibly tiered (small tenants pooled where contracts allow, large tenants siloed). This is where every serious ML SaaS converges, and it is where both a centralising design and a tenant-local design actually live. The disagreement is over millimetres of boundary placement, which is worth saying out loud occasionally to lower the temperature.

**The pattern that answers the fleet-management objection: the reconciliation/agent model.** Ship a versioned agent (executor, operator — the name doesn't matter) into every tenant. The control plane holds *desired state*: workflow definitions, model promotion state, config, target agent version. Agents *pull* desired state, reconcile local reality against it, execute work locally under local identity, and report status up. This is Kubernetes' shape, and GitOps', and it converts "N deployments to manage by hand" into "N agents converging on one declared state":

- **Upgrades** become a declaration ("fleet target: v1.14") rolled in waves — canary tenants first, watch, widen. Rollback is re-declaring the previous version.
- **Version skew** becomes *visible and bounded* rather than accidental: the control plane knows every tenant's version, and you enforce an N-1 compatibility contract on the control-plane API (expand/contract migrations: add the new field, migrate the fleet, only then remove the old) so skew is safe while it exists.
- **Credentials never travel** (Test 3): the agent holds only its own tenant's identity.
- **Outage behaviour is correct by construction** (Test 1): agents keep running last-known-good state when the control plane is down; they just stop receiving changes.
- **The pager splits sanely** (5.8): the platform team owns the control plane and agent *software*; tenant incidents are shardable.

This is, concretely, "the same service across many CPUs localised to the tenant" — the common instinct — but with the fleet-management answer built in, which is what upgrades it from instinct to architecture. When you argue for tenant-local execution, argue for *this*, and lead with the upgrade story, because that's the objection waiting for you.

---

## 8. Money: quotas, metering, and pay-per-use

The puzzlement around "quotas" dissolves once you separate three things that all get called that:

- **Metering** — measuring consumption to bill for it. Only coherent where the meter sits on something the provider actually pays for per-unit. A coordination-only control plane consumes trivially per request, so metering *it* is theatre; real pay-per-use must meter data-plane consumption (tenant compute-hours, training runs, scored rows) and report it up to a central billing capability — which is itself legitimately control-plane.
- **Rate-limiting** — protecting a shared service from overload. Entirely sensible for a pooled control plane regardless of billing model; this is often what such a quota system actually is, and it's unobjectionable framed that way.
- **Governance caps** — business limits ("your tier includes 4 retrains/month") enforced at the point of coordination. Also coherent, also control-plane, and also — note — a *control play*: caps enforced centrally are enforceable in a way per-tenant honour-systems aren't. Which is often part of the point, and is a legitimate business motive even when it's not an engineering one.

So the resolution to "does pay-per-use make sense?": the quota system makes sense as rate-limiting and governance; it makes no sense as economics unless it meters the data plane. If someone claims the central service enables usage-based pricing, the follow-up question is "what data-plane meter feeds it?" — and if there's no answer, it's governance wearing a pricing costume.

---

## 9. Scoring a centralising design

Run a central-service design through the framework, steelman first — you should be able to state its case better than its authors do before you critique it.

**Steelman (the strongest true claims):** one API surface and one place to update (even skeptics concede this has value); one definition of workflows across the fleet; a foothold for consistent onboarding, governance, and eventually billing; cheap to run because it's coordination-only; and it doesn't move data, so it respects the physics. On axes 5.2, 5.5, and 5.9 it scores genuinely well. As a *concept* it is a defensible bridge-model control plane — it works because it does not change the reality much.

**Where it actually fails the framework:**

- **Test 3, badly:** credentials in headers make it a credential-forwarding hub. Worst-case single-compromise blast radius: every tenant. This is the most objective, least political defect — lead with it.
- **Test 1, probably:** if the API is synchronously in the path of work (taking credentials per-request and doing the connecting itself), a control-plane outage stalls tenant work. Job-spec hand-off to tenant-local executors would fix both this and the credential problem at once — which conveniently is the architecture worth proposing anyway.
- **5.4:** central connection handling against N leader-node warehouses; thread-pool sprawl is the symptom, and whichever service runs the heaviest batch load is where the throttling bill likely arrives first.
- **5.5, by mandate:** if it centralises implementations rather than contracts — "you may only do what the executor supports" — it caps the fleet's capability at the platform team's throughput. Whether this happens is an execution question, not an architecture one, but where the team's prior is not favourable, it's fair to say so in the terms of Section 5.8.

Note the shape of this critique: it accepts the concept, names the specific tests failed, and proposes an adjustment (job specs + tenant-local executors + local identity) that fixes the failures while *giving the centralisers everything they legitimately wanted* — one API, one definition, one registry. That is a much harder position to dismiss than opposition, because it out-competes the proposal at its own goals. And any adjacent service that looks like "the same game" should be run through the identical tests — same wall, same questions: what plane is it, what does its outage stop, what credentials does it hold?

---

## 10. Scoring a tenant-local design

The same honesty, applied to the tenant-local instinct — know your own weak axes before someone else finds them:

- **5.2 is the exposed flank.** Without the agent/reconciliation pattern and real CI/CD investment, per-tenant execution decays into version-skew sprawl. The answer must be concrete: declared fleet state, canary waves, N-1 compatibility, a version dashboard. If you can't fund that, the central service is honestly the safer mediocrity — this is the one concession worth pre-committing to, because making it voluntarily buys credibility for everything else.
- **5.9 needs deliberate investment:** fleet-wide log/metric aggregation and trace continuity don't happen by default in a distributed design; budget them as first-class, not as a later.
- **Scale-to-zero has sharp edges** against warehouses (cold-start connection churn — the same connection observation cuts against the tenant-local design too, if workers cold-start per job). Keep warm the thin thing that holds the session; scale the workers.
- **Aggregate incident rate** will likely be higher even as each incident shrinks; make sure your SLA story is framed around correlation, where you win, not raw counts, where you may not.

---

## 11. Falsifiable predictions

Write predictions down *before* systems ship. If they come true, you gain standing no meeting-room argument can buy; if they don't, you learn where the framework misjudged. Common candidates:

1. **A heavy batch service will throttle warehouse leader nodes** via per-statement validation and connection churn in batch windows — visible as leader-node CPU saturation and customer query queuing during job runs, on Redshift-style tenants but not Postgres ones.
2. **A central API's thread/process-pool architecture will produce head-of-line blocking** under concurrent tenant load — long-running tenant work starving the event loop, p99 latency spikes uncorrelated with the *calling* tenant's behaviour (noisy neighbour, coordination-flavoured).
3. **A control-plane incident will halt tenant-side work in flight** (Test 1 failure), demonstrating the synchronous coupling — note the date and the scope when it happens.
4. **Credential handling will fail an audit or a security review** — header-borne credentials logged, retained, or over-scoped — before any actual breach forces the issue. (You want to be on record proposing the fix *before* this one.)
5. **Version skew will appear anyway**, in whatever the central service doesn't cover — because a partial control plane centralises the easy layer and leaves the fleet problem where it always was.

Keep the list honest: also record predictions that *fail*. A framework you only use to confirm yourself is a grudge with headings.

---

## 12. Paved roads vs toll gates

The organisational layer, faced directly. Platform teams succeed in one of two modes:

- **Paved road:** the central platform wins adoption by being better than what teams would build themselves — better docs, better reliability, less friction. Teams adopt because deviating is more work. Adoption is the *metric* of quality.
- **Toll gate:** the platform wins adoption by decree. Mandate substitutes for quality; the feedback loop that would improve the platform is severed, because users can't leave and their pain doesn't price in. Quality then drifts wherever the team's internal standards let it drift — which is the toll-gate failure in one line: the idea could be fine, but do a shit job and then force everyone to put up with it, and that's just toxic.

The litmus test to carry into every meeting: **would teams choose this service if it were optional?** Note what this test does *not* say — it doesn't say platforms must literally be optional forever (some things, like security boundaries and billing, legitimately end up mandatory). It says mandate should *follow* demonstrated quality, not substitute for it. A fair sequencing demand, politically speaking: make it optional for two quarters, instrument adoption, mandate only what teams already chose. If the platform team refuses that sequencing, they are telling you which mode they're in.

And name the underlying law: **Conway's law runs both ways.** The architecture will mirror the org chart — a central-services push is also a centralisation of decision rights, which is why reading it as "a control play" is accurate rather than cynical. But control planes are, definitionally, control plays. The question is never whether control is being taken; it's whether it is being taken *over the right plane* (contracts: yes; implementations: no) and *exercised competently* (paved road vs toll gate). Those two questions — plane and competence — are separable, and keeping them separate is what lets you oppose a team's execution without opposing an architecture you'd endorse from a team you trusted.

---

## 13. Arguing well: keeping the critiques separate

Three registers, three standards of proof, three audiences:

- **Physics claims** ("the data can't leave the tenant") — provable, uncontroversial, and shared ground with the other side. Spend these freely; they cost nothing.
- **Design claims** ("credentials shouldn't converge on one service"; "the control plane must be off the hot path") — arguable from the tests and axes above, winnable on merits, and *independent of who builds it*. This is where the framework does its work: a design claim that only holds if the team is weak is not a design claim.
- **Execution/org claims** ("this team will build it badly and mandate it anyway") — real, and often evidenced, but the weakest register to lead with, because it reads as (and partly is) a trust dispute. Deploy it only where the framework says team quality is a legitimate input: blast-radius concentration (5.8) and the paved-road sequencing demand (12).

The failure mode to avoid is arguing register three in the costume of register two — opposing the architecture because you distrust the architects. The framework's discipline is the inverse: concede the concept loudly (bridge model, thin control plane — sound), attack the specific test failures precisely (credentials, hot path, connection economics), propose the variant that fixes them while serving the centralisers' own stated goals (job specs, tenant agents, central registry), and reserve the trust critique for the one place it is engineering rather than grievance: *how much correlated blast radius should this particular team be allowed to hold?*

That position is very hard to beat. It also has a property worth wanting for its own sake: it stays true no matter who ends up building the thing.

---

## 14. Checklists

**Questions to ask of any centralisation proposal:**

1. Which plane is this capability in — and does the proposal agree with its own answer?
2. What stops when it's down for an hour? (Test 1)
3. Do rows ever transit it? Do aggregates that reconstruct rows? (Test 2)
4. Where do credentials live, travel, and converge? What's the worst single compromise? (Test 3)
5. Is it worth more because there's exactly one? (Test 4)
6. Does it centralise contracts or implementations?
7. Is it on any hot path?
8. What's the fleet-management story it replaces or requires?
9. What does the quota actually meter — and does a data-plane meter feed the billing claim?
10. Would teams adopt it if optional — and if mandatory, was quality demonstrated first?

**Red flags:** credentials in transit through shared services; control-plane calls per-request rather than per-deployment; "the executor only supports…" as an answer to a capability question; quotas justified as pricing with no data-plane meter; mandates preceding adoption metrics; connection objects for N tenants held in one process; validation chatter against leader nodes; telemetry that includes raw feature values.

**Green flags:** job specs without secrets; agents pulling desired state; artefact pointers in the registry, bytes in tenant S3; N-1 compatibility promises with expand/contract migrations; canary-wave rollouts with a fleet version dashboard; control-plane outages that tenants don't notice; a platform team that publishes adoption numbers voluntarily.
