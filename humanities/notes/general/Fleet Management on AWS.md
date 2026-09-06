*This is a companion to the Central Services framework essay. Fleet management is the exposed flank of any tenant-local architecture, and the agent/reconciliation pattern is the answer. This essay makes that answer concrete on AWS for a common setup: an AWS account per tenant, tenant services on EKS. The goal is that "the same service across many CPUs localised to the tenant" becomes an operating model you could put in front of a platform team.*

---

## 1. What "industry-grade" actually means

Before any AWS service names: fleet management is a set of *capabilities*, and a design is industry-grade when it has all of them. This list is the definition of done — and, usefully, an audit checklist for anyone else's proposal.

1. **Declared desired state.** For every tenant, the intended version of every component, its configuration, and its wave/tier membership exist as data in one place — not in the heads of operators or the history of a deploy tool.
2. **Total fleet visibility.** One query answers: what version is each tenant running, when did it last reconcile, is it healthy, and does reality match declaration? If you cannot see the fleet, you do not manage it — you manage incidents.
3. **Wave-based rollout with automated gates.** Changes reach the fleet in ordered waves with bake time between them, automated health evaluation at each gate, and automatic halt on regression. Nobody "deploys to all tenants"; they *declare* a target and the machinery walks the fleet.
4. **Rollback as re-declaration.** Undo is declaring the previous known-good state, not an artisanal recovery. If rollback requires a runbook longer than a page, it isn't rollback.
5. **Bounded, visible version skew.** Skew exists during every rollout — the system makes it safe (compatibility contracts) and visible (the fleet dashboard), rather than pretending it away.
6. **Drift detection and reconciliation.** Out-of-band change — a console edit, a hotfix applied by hand — is detected and either reverted automatically or flagged loudly. The fleet converges on declared state continuously, not just at deploy time.
7. **Zero-touch tenant lifecycle.** Onboarding a tenant is a pipeline, not a project: account, baseline, stack, registration, smoke test. Offboarding is equally mechanical.
8. **No credential convergence.** The machinery that manages N tenants must not itself become a credential-forwarding hub whose compromise yields access to every tenant. This is the constraint most fleet designs quietly violate.
9. **Fleet-wide observability with tenant-level drill-down.** Aggregated metrics, logs, and traces in one place, sliceable by tenant, without customer data leaving the tenant — telemetry is metadata, and must stay that way.
10. **A complete audit trail.** Every change to every tenant answers who, what, when, why — from Git history and pipeline logs, not from memory.

Everything below is the AWS-shaped implementation of these ten. A useful mental model for the whole design: **Git is the desired state, the fleet registry is the index, tenant-local reconcilers are the muscle, and the control plane never touches a tenant credential.**

---

## 2. The shape: hub and spokes

The account topology mirrors a two-plane architecture — a data plane with gravity and a thin control plane:

- **N tenant accounts** (the spokes) — the data plane. Each holds its own EKS cluster (or shares a cluster per tenant boundary — see 2.1), warehouse, S3, models, secrets, KMS keys. Everything with gravity.
- **A small set of central accounts** (the hub) — the control plane, deliberately split so no single account accumulates god-powers:
  - **Artefact account** — ECR registries, Helm charts, signed images. Write access: CI only. Read access: every tenant account.
  - **Deployment/config account** — the Git remote (or its AWS mirror), the fleet registry, the orchestration that advances waves.
  - **Monitoring account** — the observability sink (CloudWatch cross-account observability, or a Prometheus/Grafana stack). Read-side only.
  - **Security/audit account** — org CloudTrail, Security Hub, GuardDuty findings. Owned outside the platform team if possible; the auditors of the machinery should not be the operators of it.
  - **Management account** — AWS Organizations root. Touched rarely, by almost nobody, for org structure and SCPs only.

This split is itself a no-convergence discipline: compromising the artefact account gets you the ability to publish images (mitigated by signing, Section 6), not the ability to read a warehouse. Compromising monitoring gets you metrics. No hub account holds tenant data-plane credentials — Section 4 is the mechanism that makes this true.

### 2.1 The account boundary is the whole game

Account-per-tenant is the strongest isolation primitive AWS offers — it is simultaneously the IAM boundary (cross-account access is deny-by-default and must be explicitly granted twice, on both sides), the blast-radius boundary, the quota boundary (one tenant's API throttling can't starve another), the billing boundary (per-tenant COGS falls out of Cost Explorer for free), and the KMS boundary (tenant data encrypted under tenant-account keys). Every design decision below leans on this. If anyone ever proposes consolidating tenants into shared accounts "to simplify," the honest translation is "to weaken all five boundaries at once to save some Organizations tooling" — occasionally worth it at the very small-tenant tier (a pooled tier for tiny tenants), never as the default.

**Organizations structure that earns its keep:** arrange tenant accounts into OUs that encode *operational* facts, not org-chart facts — by environment (sandbox/staging/prod), and within prod by rollout wave and/or tier. OU membership then does real work: Service Control Policies apply guardrails per OU (Section 12), and wave membership is legible in the org tree itself. Use Control Tower (or a leaner hand-rolled landing zone if Control Tower feels heavy) to get account baselining, centralised CloudTrail, and guardrails without building them yourself; **Account Factory for Terraform (AFT)** if you take the Terraform route in Section 10, so account creation is itself pipeline-driven (capability 7).

---

## 3. The fleet registry

The heart of the system, and deliberately boring: a table. DynamoDB is the natural fit (serverless, single-digit-ms, and the access pattern is key-value by tenant), though Postgres in the deployment account is fine too. One row per tenant, roughly:

```
tenant_id            acme
aws_account_id       123456789012
tier                 enterprise | standard | starter
environment          prod
wave                 3                      # rollout wave membership
region               eu-west-2
components:
  agent:             { current: 1.13.2, target: 1.14.0 }
  forecast-service:  { current: 2.4.0,  target: 2.4.0 }
  ingestion:         { current: 5.1.1,  target: 5.1.1 }
config_hash          9f2a…                  # hash of rendered config, for drift comparison
last_reconcile       2026-07-18T09:41:00Z
health               green | degraded | failed
freeze               none | until:2026-07-21  # change freeze (customer demo, peak period)
flags:               { new_scheduler: true, … }
```

Three rules give it teeth. **Everything reads it:** the rollout orchestrator (which tenants are in wave 3?), the GitOps generator (Section 7 — one Application per row), the dashboard (Section 11), the vending pipeline (Section 12 writes the row), support tooling ("what's tenant X running?"). **`current` is written only by telemetry** — reported by the tenant-side reconciler, never assumed by the deployer; the gap between `target` and `current` *is* the fleet's rollout state, and observed-not-asserted is what makes the dashboard trustworthy. **`target` is written only by the rollout machinery** — humans change targets by pull request, not by console.

This registry is where the control-plane promises become real: it is pure metadata, it fails safe (registry down → fleet keeps running current versions), and it is the strong metadata registry a central service legitimately should be — an index, not a credential hub.

---

## 4. Identity: managing N tenants without holding N credentials

The critical section. Fleet machinery is exactly the kind of component that drifts into credential-hub status, because the lazy implementation — a deployer that holds admin credentials for every tenant — works fine right up until it's the worst thing you own. The AWS-native answer is that **nothing ever holds long-lived tenant credentials, because identity is assumed at point of use and expires in minutes.**

**CI to AWS: OIDC federation, no stored keys.** GitHub Actions (or GitLab/CodeBuild) authenticates to AWS via OIDC — the pipeline presents a short-lived identity token and assumes a role; there are no AWS access keys in CI secrets at all. The trust policy pins the exact repo and branch, so only `main` of the infrastructure repo can assume the deployment role.

**Hub to spoke: narrow, auditable role assumption.** Each tenant account contains a small set of roles created at vending time, each with a tightly scoped trust policy back to one specific hub principal: an *infrastructure* role assumable only by the IaC pipeline (Section 10), a *break-glass* operator role assumable only via the identity provider with MFA, logged loudly. Note what's absent: no role assumable by "the platform" broadly, and — most importantly — **no runtime path from hub to spoke at all** for the deployment of application software, because of the next point.

**The reconciler pulls; the hub never pushes.** This is the architectural decision that kills credential convergence for day-to-day operations. In a push model (including hub-and-spoke Argo CD, where a central Argo holds API credentials for every tenant cluster — see Section 7's honest comparison), the hub must hold reach into every spoke, and you're back to a convergence point. In the pull model, the only standing cross-account grants are *read-only and inward*: tenant accounts may pull from central ECR and Git. A compromised hub can publish a malicious artefact — mitigated by signing and admission control (Section 6) — but cannot reach into a tenant. The worst-case single compromise stops being "every warehouse" and becomes "the supply chain," which is a defendable, instrumentable surface.

**Inside the tenant: pod-level identity, local secrets.** Workloads on EKS get AWS permissions via IRSA or EKS Pod Identity — each service account maps to an IAM role in the *tenant's own account*, scoped to that tenant's S3, that tenant's secrets. Warehouse credentials live in the tenant account's Secrets Manager, rotated there, read at runtime by the tenant's own pods, encrypted under the tenant's KMS key. They never appear in Git, never transit the hub, never ride a request header. This, concretely, is the fix for the credential-forwarding anti-pattern.

---

## 5. One build, immutable artefacts, promotion not rebuild

Fleet management collapses if "version 1.14.0" can mean different bytes in different tenants. The chain:

- **Build once.** CI produces one container image per component per commit, pushed to central ECR in the artefact account. Tenants pull cross-account (ECR resource policies grant org-wide read via the `aws:PrincipalOrgID` condition — one policy, not N) or via ECR replication into tenant regions if pull latency/egress matters.
- **Deploy digests, not tags.** Tags are mutable pointers; digests are content addresses. The registry's `target: 1.14.0` resolves — once, centrally — to an image digest, and the rendered manifests pin the digest. "What is tenant X running" then has a cryptographic answer, and a re-pushed tag can't silently change the fleet.
- **Sign at build, verify at admission.** Sign images in CI (cosign, or AWS Signer's container signing); in each tenant cluster, an admission controller (Kyverno's image-verification policies are the usual choice) refuses any pod whose image isn't signed by your CI key *and* pulled from your ECR. This is the mitigation that makes the pull model's residual risk acceptable: a compromised artefact account alone can't get unsigned code onto the fleet, because admission verifies signatures against a key the artefact account doesn't hold. Add SBOM generation in CI for the day a CVE question arrives as "which tenants run the affected library?" — which becomes a registry query instead of an archaeology project.
- **Charts and config are artefacts too.** Helm charts pushed to ECR as OCI artefacts, versioned and signed like images; rendered per-tenant config hashed into the registry's `config_hash`.
- **Model artefacts are the exception that proves the rule.** Model bytes stay in tenant S3, and the central model registry holds pointers and hashes. The fleet machinery ships *code and config* to tenants; it never ships *data or models* between them.

---

## 6. Desired state and reconciliation on EKS

On EKS, the mature version of the reconciliation agent is not something you write — it's GitOps tooling you operate. Two credible shapes, and the choice matters:

**Option A — hub-and-spoke Argo CD.** One central Argo CD in the deployment account manages all tenant clusters remotely. Genuinely attractive: one pane of glass, ApplicationSets fan out configuration beautifully, and it's the shape most platform teams reach for first. But look at it through the convergence lens: the hub Argo holds API-server credentials for *every tenant cluster*, and its UI/API becomes a single component whose compromise yields write access to the whole fleet. It also fails the availability test partially — hub down means no tenant can converge. It is, precisely, a credential-forwarding hub with a nice UI.

**Option B — a reconciler per tenant (recommended).** Each tenant cluster runs its own lightweight GitOps agent — Flux, or a small per-tenant Argo — which *pulls* its declared state from the central Git repo/OCI registry and converges its own cluster. The hub holds no cluster credentials (Section 4); a control-plane outage leaves every tenant converging on last-known-good, by construction; the blast radius of a compromised reconciler is its own tenant. The cost is honest: N reconciler installations to keep patched — but the reconciler is itself just another fleet-managed component, upgraded through the same waves as everything else, so the machinery manages the machinery. Flux's OCI support fits particularly well here: tenants pull signed config bundles from ECR, so Git itself needn't be reachable from tenant runtime at all.

**How a tenant knows what "its declared state" is:** a generator in the deployment account renders per-tenant desired state from three layered inputs — fleet-wide base (one Helm chart / kustomize base per component), tier overlay (enterprise vs starter sizing, features), tenant overlay (the registry row: versions, flags, config) — and publishes the result to a per-tenant path in Git or a per-tenant OCI artefact. Layering is what prevents the N-snowflakes decay: a tenant's *entire* deviation from the fleet norm is its registry row and a (usually tiny) overlay file, both reviewable, both diffable. If a tenant's overlay grows large, that's a smell with a name — you're re-deriving the museum of versions — and it's visible in code review rather than discovered during an incident.

**In-tenant progressive delivery:** within each tenant, Argo Rollouts (or Flux's Flagger) canaries new versions against live traffic — shift 10%, compare error rate and latency against the stable baseline, promote or auto-rollback. So the fleet has *two nested safety loops*: across tenants (waves, Section 7) and within each tenant (canary analysis). A bad version has to fool both to hurt anyone broadly.

---

## 7. Rollout mechanics: waves, gates, and the N-1 contract

**Waves.** Rollouts walk the fleet in registry-defined waves: wave 0 is internal/demo tenants, wave 1 a handful of friendly canary tenants, then progressively broader, with the most change-sensitive enterprise tenants last. The rollout orchestrator — a Step Functions state machine in the deployment account is a natural fit, EventBridge-triggered, and note that it only ever *writes `target` fields in the registry*; the tenants' own reconcilers do the deploying, so the orchestrator needs no tenant access (Section 4 holds) — advances as: set `target` for wave k → wait for `current` to converge (telemetry, not assumption) → bake for a defined soak period → evaluate gates → proceed, or halt and revert targets.

**Gates are queries, not opinions.** A gate is an automated evaluation over the monitoring account's data, scoped to the wave: error rates vs pre-rollout baseline, latency percentiles, reconcile failures, component-specific SLIs (for the ML estate: scoring job success rate, drift-metric deltas, inference p99). Define them per component *before* the rollout, in code, next to the chart. A halted rollout pages a human; a passed gate does not need one.

**Bake time is a feature, not slowness.** The instinct that fleet-wide same-day deployment is the goal is wrong — correlated failure is the thing the whole architecture exists to avoid, and deliberate wave spacing is what de-correlates it. A revision that reaches the last wave three days after the first, having been continuously health-checked, is *faster* in the metric that matters: time-to-safely-everywhere. Keep an explicitly separate **fast lane** for security patches — same waves, compressed bake times, pre-agreed — so that emergencies don't become the excuse that erodes the normal lane. And honour `freeze` rows: tenants in a freeze window are skipped and reconciled when the window ends, with the skew that creates made visible on the dashboard rather than forgotten.

**Version skew is a contract, not an accident.** During every rollout the fleet is mixed-version by design, so compatibility is a tested promise: every component supports N-1 (at minimum) against the control-plane API and its own consumers, enforced by contract tests in CI that run the previous released version against the new one *before* merge. Schema and API changes ship expand/contract (add the new field, migrate the fleet, only then remove the old — never break-and-fix), which is what makes "tenant 14 is three versions behind" a dashboard fact rather than an outage. Skew stops being the reason to centralise once it is bounded, visible, and contractually safe.

**Rollback** is the orchestrator re-declaring the previous digests for a wave (or the fleet) and letting reconcilers converge — same machinery, opposite direction, no special path. Practice it on purpose, in a game day, before you need it in an incident; rollback that has never been rehearsed is a hypothesis, not a capability.

---

## 8. Configuration and flags

Two kinds of config, two paths. **Structural config** (resources, topology, versions, env vars) travels through the GitOps path with everything else — it *is* desired state, and drifts are caught by the same reconcilers. **Dynamic config and feature flags** (kill switches, gradual behavioural rollout, per-tenant enablement) need to change faster than a deploy: AWS AppConfig is built for exactly this — validated configuration (schema or Lambda validators, so a bad flag value is rejected at publish), deployed *gradually* with automatic rollback tied to CloudWatch alarms. Layer it the same way as manifests: fleet defaults, tier overrides, tenant overrides, with tenant-level flags mastered in the fleet registry so the dashboard can answer "which tenants have `new_scheduler` on?" — because a flag nobody can enumerate is skew in disguise. One discipline: flags are for *behaviour*, versions are for *code*; the moment a long-lived flag is choosing between two large code paths per tenant, you've rebuilt version skew inside the binary, with none of the tooling.

---

## 9. The infrastructure layer beneath the clusters

GitOps converges what's *inside* clusters; something still has to manage the accounts, VPCs, EKS control planes, warehouses, and IAM roles themselves. Given "IaC: not sure," this is a decision to make deliberately rather than inherit:

- **Recommendation: Terraform (or OpenTofu), organised as a small number of versioned root modules** — `tenant-baseline` (account plumbing, roles from Section 4, guardrails), `tenant-eks`, `tenant-data` (warehouse, S3, KMS) — with one thin, parameter-only instantiation per tenant. The same anti-snowflake layering as Section 6: a tenant's infrastructure identity is a short tfvars file, and module version bumps roll through the same wave discipline as application code. Per-tenant state files (S3 backend in the deployment account, tenant-scoped), applied by the OIDC pipeline assuming each tenant's infrastructure role — infrastructure change is push-based by nature, but it's *pipeline*-push with short-lived credentials and full plan/apply audit, which is acceptable where runtime push wasn't. AFT ties this to account vending.
- CDK/CloudFormation with StackSets is the AWS-native alternative — StackSets' auto-deploy-to-OU is genuinely elegant for baseline guardrails even in a Terraform shop (using both is common: StackSets for org-level baseline, Terraform for tenant stacks). Crossplane — managing AWS resources from inside Kubernetes via GitOps — is worth knowing exists, but adopting it while also standing up fleet management is two novelties at once; revisit later.
- **Drift:** scheduled `terraform plan` across the fleet with non-empty plans surfacing on the dashboard, plus AWS Config rules in every tenant account for the guardrail-grade invariants (encryption on, public access off, roles unmodified). Manual console mutation in tenant prod accounts should be *impossible* for routine roles (SCPs, Section 12) and *loud* for break-glass.

---

## 10. Observability: seeing the fleet as one thing

Fleet-wide observability is the one capability a distributed design must deliberately *build* rather than inherit; here is what building it means. **CloudWatch cross-account observability** links every tenant account as a source to the monitoring account as a sink — metrics, logs, and traces queryable centrally without per-account hopping. The alternative stack — ADOT (OpenTelemetry) collectors in each cluster shipping to Amazon Managed Prometheus/Grafana — is more work and more capability; either satisfies the requirement, and both keep customer data in the tenant *only if you police it*: telemetry pipelines must carry metrics, statuses, and structured events — never rows, never feature values (log-scrubbing at the collector, and code review treats a logged payload as a security bug, because it is).

Three views matter more than any dashboard aesthetics. **The fleet grid:** every tenant × every component — `current` vs `target`, last reconcile, health, wave, freeze — i.e., the registry, rendered; this is the single artefact that most changes executive conversations about the tenant-local model, because it makes N deployments *look* managed. **Rollout view:** the active rollout's wave progression, gate evaluations, and the baseline-vs-canary metric comparisons that gates are deciding on. **Tenant drill-down:** one tenant's logs, traces (trace IDs propagate from any control-plane touchpoint through tenant-side work, so a support question walks one trace, not two systems), and reconcile history. Plus the heartbeat convention that feeds it all: every reconciler and every fleet-managed service emits a periodic structured heartbeat — version, config hash, health — which is what writes `current` and `last_reconcile` (Section 3's telemetry-only rule).

---

## 11. Tenant lifecycle as a pipeline

Onboarding, fully mechanised, is where the account-per-tenant model pays for itself — and it's a Step Functions pipeline, not a project plan: create the account via AFT into the correct OU → baseline lands (CloudTrail to audit, guardrails, the Section 4 roles, monitoring link) → Terraform applies `tenant-eks` and `tenant-data` → the registry row is written (wave, tier, initial targets) → the generator emits the tenant's desired state → the tenant's fresh reconciler converges it → smoke tests run against the new stack → the dashboard shows a new green row. Time-to-tenant becomes a metric you can quote in sales conversations, and — worth noticing — this pipeline out-competes the central-services pitch at one of its own stated goals (consistent onboarding) while strengthening isolation rather than weakening it. Offboarding is the mirror: freeze, final data export to the customer, retention clock, account close — mechanical, auditable, and contractually legible because the account boundary makes "delete the tenant" a well-defined operation.

---

## 12. Guardrails, audit, and the security spine

- **SCPs by OU:** deny leaving the org, deny CloudTrail tampering, deny disabling encryption, deny console mutation of protected roles; prod-OU policies stricter than sandbox. SCPs are the "even root in the tenant account can't" layer — the guarantee that drift-by-hand is structurally hard, not just discouraged.
- **Org-wide CloudTrail** into the security account: every role assumption from Section 4, every break-glass use, every pipeline apply — one queryable audit plane. Combined with Git history and registry history, capability 10 (who/what/when/why) is answered by construction.
- **GuardDuty + Security Hub** across all accounts, findings aggregated to the security account; IAM Access Analyzer to catch any cross-account grant that isn't one of the deliberate ones from Section 4 — the automated watchdog for exactly the credential-convergence regression this design exists to prevent.
- **Supply-chain posture recap** (because it's the pull model's residual risk): OIDC-only CI, digest pinning, signing at build, admission verification in-tenant, SBOMs, ECR scanning. The fleet's attack surface is deliberately concentrated here because it's the surface you can instrument and test, unlike N ad-hoc SSH paths.

---

## 13. Cost: the boring superpower

Account-per-tenant means per-tenant COGS is a Cost Explorer filter, not a data-science project: exact per-tenant infrastructure cost, gross margin per customer, and cost-anomaly alerts per account (a runaway training job shows up as *that tenant's* anomaly, not fleet noise). This is also ammunition for the cost argument — with real per-tenant numbers, nobody gets to hand-wave that centralisation would be cheaper; you can price the comparison. Scale-to-zero economics apply within each tenant: Karpenter for node autoscaling to near-zero between batch windows, Fargate profiles for spiky small workloads, with the previously-noted caveat — keep warm whatever holds warehouse sessions; scale the workers, not the session-holder.

---

## 14. Build order: visibility first, control second

Sequencing matters more than completeness — each stage pays for itself before the next starts, which is also what makes this fundable politically:

1. **The registry and the fleet grid** (weeks, not months): even populated by a script that *observes* current versions, visibility alone changes every conversation — including the central-services one, because "we can't see the fleet" is that proposal's best tacit argument, and this deletes it.
2. **The artefact chain:** one build, central ECR, digest pinning, signing. No behaviour change for tenants yet; every later stage depends on it.
3. **Per-tenant reconcilers + the generator:** tenants converge on declared state; drift dies; deploys become PRs. Start with wave 0 (internal tenants) only.
4. **Wave orchestration with gates:** the Step Functions walker, gate queries, the fast lane, rehearsed rollback.
5. **Vending pipeline + guardrail hardening:** the lifecycle and security spine, which by now mostly formalises what stages 1–4 established.

The honest resourcing caveat still applies: this is a real platform investment — if the organisation won't fund roughly stages 1–3, then per-tenant execution *will* decay into the museum of versions, and a central service becomes the safer mediocrity. But notice the framing this earns: stages 1–3 are *smaller* than a competently-built central execution service, they preserve every isolation property the business sells, and stage 1 is cheap enough to simply do — at which point the fleet grid exists, and arguments about manageability get conducted while looking at it.

---

## 15. The properties that matter

Read the design against the four properties any tenant-local control plane has to earn. **Graceful failure:** the control plane here (registry, Git, artefacts, orchestrator) fails safe — its total outage stops *change*, never *work*. **Boundary discipline:** only code, config, and telemetry cross the tenant boundary — never rows, never models. **No credential convergence:** this is the design's spine — pull-based reconciliation plus OIDC plus tenant-local secrets means no component's compromise yields fleet-wide data access, with the supply chain as the deliberately-instrumented residual surface. **Central by uniqueness:** exactly the uniqueness-valuable things sit centrally (declarations, artefacts, observability, audit) and everything else is replicated per tenant. The green flags follow from the same structure — job specs without secrets, agents pulling desired state, N-1 promises, canary waves, a fleet version dashboard, control-plane outages tenants don't notice — and map line-for-line onto Sections 4, 6, 7, and 10. That is the position to argue from: not "against central services," but *"here is the control plane I'd build — thinner, safer, and it shows you the whole fleet on one screen."*
