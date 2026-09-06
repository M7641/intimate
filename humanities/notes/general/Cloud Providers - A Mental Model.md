# Cloud Providers — A Mental Model

*Written from the metal up. The aim is that nothing in AWS (or Azure, or GCP) should feel arbitrary by the end — every strange-looking primitive, IAM roles very much included, should feel like the inevitable consequence of a small number of physical, economic, and security constraints. Part I builds the model in layers. Part II compresses it into the recurring design principles. Part III maps it onto your own estate, and Part IV is the structured learning path.*

---

# Part I — The model, from the metal up

## 0. The one-sentence model

> **A cloud provider is a planet-scale, multi-tenant computer, rented by the second, operated entirely through an authenticated API.**

Every word in that sentence forces a piece of the architecture you see:

- **Planet-scale** → physics and law intrude, so the world is carved into *regions* and *availability zones*, and almost nothing is truly "global."
- **Multi-tenant** → millions of mutually distrustful customers share the same hardware, so *isolation* is the provider's core engineering problem, and you get accounts, VPCs, KMS, hypervisors, and the rest of the isolation stack.
- **Rented by the second** → elasticity is the economic product, so pricing, autoscaling, spot markets, and "pay for what you provision vs what you consume" all follow.
- **Operated entirely through an API** → there is no "inside." The console is just another API client. Which means every single action must be authenticated and authorised — and *that* is why IAM exists, why it is shaped the way it is, and why it sits underneath everything else rather than off to one side.

Hold onto this sentence. Each section below is one clause of it, unpacked.

---

## 1. The physical layer: regions, availability zones, and why geography won't go away

Start with what a cloud provider physically *is*: warehouses full of servers, bought in bulk, wired together with a private global network, plus power contracts, generators, and cooling. The marketing word "cloud" hides this, but the physical layer leaks through into everything you touch, so it's the right ground floor.

**A data center fails as a unit.** Power distribution, cooling, fire suppression, the network spine — these are shared within a building, so failures inside a building are *correlated*. A single data center, however well built, can be taken out by one backhoe, one fire, one bad switch firmware push. If you want to offer customers "your service stays up," you cannot do it with one building.

**So: the availability zone (AZ).** An AZ is one or more data centers with independent power, cooling, and networking, close enough to its siblings for fast synchronous communication (single-digit millisecond, close enough to replicate a database write before acknowledging it) but far enough apart that one flood or grid failure doesn't take out two. The AZ is the provider's *unit of correlated failure*. Everything AWS tells you about high availability reduces to: **run in at least two AZs, because anything within one AZ can die together.**

**A region is a cluster of AZs** (usually three or more) in one geographic area. Why do regions exist at all, rather than one giant world-computer?

1. **Physics.** The speed of light is ~200,000 km/s in fibre. London to Sydney and back is ~150ms *at minimum*, before a single router. Synchronous replication across that distance is impossible for interactive systems; you must place compute and data near users.
2. **Law.** Data-residency and sovereignty rules (GDPR being the one that bites you) mean customers need to *prove* data stays in a jurisdiction. A region is a legal artefact as much as a technical one.
3. **Blast radius.** Regions are designed to fail *independently* — separate control planes, separate deployments. When AWS pushes bad code (it happens; us-east-1 has the scar tissue to prove it), the failure is meant to stop at the region boundary. Regions are the provider's own largest unit of "don't let one mistake take out everything."

**The consequence you feel every day: services are regional by default.** An EC2 instance, an S3 bucket, an EKS cluster, a Redshift cluster — each lives in exactly one region, and "the same thing in another region" is a *separate copy you must build and reconcile yourself*. This is not laziness; it's honesty. The provider refuses to paper over the speed of light, because the papering-over (cross-region replication, failover, consistency) has real costs and real failure modes that *you* need to own decisions about.

**And the exceptions prove the rule.** A few services present as *global* — IAM, Route 53, CloudFront. Look at what they have in common: they are small, metadata-heavy, read-mostly, and needed everywhere before anything else can work (you can't authenticate a call to eu-west-2 if your identity only exists in us-east-1). Global-ness is bought with a hidden price: **eventual consistency**. When you create an IAM role, it *propagates* around the world over seconds. This is why a freshly created role sometimes fails to work for a moment — you are watching planetary replication happen. Nothing in the cloud is magic; anything "global" is a replication system wearing a trench coat.

> **Mental model to keep:** AZ = unit of correlated failure. Region = unit of independent failure + jurisdiction. Global = replicated metadata, eventually consistent. When you see any HA/DR feature, ask "which of these boundaries is it protecting me across?" — the feature will instantly make sense.

---

## 2. The founding move: everything is an API

Here is the single most important historical fact for your mental model. In the early 2000s, Amazon internally mandated (the famous Bezos memo) that all teams expose their systems to each other *only* through service interfaces — no shared databases, no backdoors, no exceptions — and that these interfaces be designed as if they might one day be exposed to the outside world. A few years later the same posture went external: S3 and EC2 launched in 2006 — not, despite the popular myth, Amazon's spare retail capacity or internal interfaces opened up, but new products built by dedicated teams in the spirit of that mandate, distilling what Amazon had learned about running infrastructure.

This origin explains the cloud's deepest structural property: **there is no "inside."**

- The AWS web console is not a privileged control panel. It is a JavaScript application calling the same public APIs you can call with `curl`. Anything you can click, you can script; anything you can't do via the API, doesn't exist.
- This is what makes **Infrastructure as Code possible at all**. Terraform, CloudFormation, CDK are not clever hacks — they are the *natural* way to use a system whose only interface is an API. The console is the anomaly, tolerated for exploration and emergencies.
- It's also what makes the cloud *composable*: every service can orchestrate every other service, because they all speak the same substrate (signed HTTPS calls). EventBridge triggering Lambda triggering Step Functions manipulating EC2 — that's just API calls calling API calls.

**The two-plane split.** Because everything is an API, every service naturally divides into:

- a **control plane** — the APIs that create, configure, and destroy resources (`RunInstances`, `CreateBucket`, `CreateDBInstance`). Low volume, high consequence, strongly audited.
- a **data plane** — the path your actual work travels (packets through the VPC, GETs against S3 objects, queries against the database). High volume, latency-critical.

Providers engineer these to fail independently, and specifically so that **control-plane outages don't stop the data plane**: when the EC2 API is down, *running instances keep running*; you just can't launch new ones. You'll recognise this in any well-designed fleet-management system: it demands the same property of its own control plane ("registry down → fleet keeps running current versions"). The same forces produce the same shapes.

**And now the crucial consequence.** If the *only* way to do *anything* is an API call arriving over a network, then the provider has no other line of defence. There's no office door, no VPN perimeter, no "trusted internal network" — a request from your CI system, from a pod in your cluster, from a laptop in a café, all arrive the same way. Therefore:

> **Every single API call, with no exceptions, must carry proof of who is calling, and be checked against a policy of what they're allowed to do.**

That sentence is the entire reason IAM exists, and the reason it's the first thing you hit and the last thing you master. Identity is not a feature of the cloud. **Identity is the perimeter.** We'll spend Section 4 there — but one more layer of foundation first.

---

## 3. The substrate: virtualization, multi-tenancy, and isolation as *the* product

The economics of cloud only work because of multi-tenancy: thousands of customers' workloads packed onto shared machines, smoothing out each other's idle time. Amazon buys servers at a scale and price you can't, keeps them busier than you would, and rents slices. The enabling technology is **virtualization**: a hypervisor slicing one physical machine into many virtual ones, each believing it owns the hardware.

But multi-tenancy creates the cloud's defining tension: **your workload is running on the same physical silicon as a stranger's — possibly a competitor's, possibly an attacker's.** The provider's most fundamental promise, the one the entire business rests on, is that tenants cannot see or affect each other. Every isolation mechanism you meet in AWS is this one promise, enforced at a different layer:

| Layer | Isolation mechanism |
|---|---|
| Silicon / hypervisor | Nitro (AWS's custom cards + minimal hypervisor) — network, storage, and security enforcement offloaded to dedicated hardware, so even a hypervisor escape hits another wall |
| API / ownership | **The account** — the hard wall around everything; nothing crosses it unless explicitly granted *by both sides* |
| Network | **The VPC** — your private network illusion, rebuilt in software (Section 5) |
| Data at rest | **KMS** — encryption keys scoped to you, usable only via policy-checked API calls |
| Identity | **IAM** — every call checked against your policies, denied by default |

Two things to internalise from this table:

**First: the account is the primitive, and it's stronger than people treat it.** An AWS account is not a billing bucket that happens to hold resources; it is the *strongest isolation boundary the platform offers* — simultaneously the identity boundary (cross-account access is deny-by-default and needs explicit grants on both sides), the blast-radius boundary, the quota boundary, and the billing boundary. This is why mature organisations run *many* accounts (per team, per environment, per tenant) under AWS Organizations, and why an account-per-tenant setup is the textbook-strong version of the pattern. If you've ever wondered "why would anyone want lots of accounts?", this is the answer: because the account is the one wall that everything respects at once.

**Second: deny-by-default is not a policy choice, it's a survival requirement.** In a single-tenant world (your own datacenter), things default to open and you close what's dangerous. In a multi-tenant world, one default-open anything is a breach of the foundational promise, at planetary scale. So the entire cloud inverts: **nothing can talk to anything until someone writes down that it may.** Every "why is this so fiddly, why do I need to grant *that* as well" moment you've ever had in AWS is this inversion. It's not bureaucracy; it's the only stable posture for a shared computer.

With that, you have the three foundations: geography carves the world into failure domains (§1), the API makes identity the perimeter (§2), and multi-tenancy makes isolation the product (§3). Now we can do IAM properly — and it will land differently than it does in the documentation, because you now know *what problem it's solving*.

---

## 4. Identity: the deep dive

This section is the heart of the guide. IAM feels arbitrary when learned as a pile of vocabulary (users, roles, policies, trust relationships, instance profiles, STS…). It stops feeling arbitrary the moment you see it as the answer to one question, asked under the constraints of Sections 2 and 3:

> *"Millions of tenants share one API-driven computer. Every action is a network call. How does the computer decide, billions of times per second, whether a given call is allowed?"*

### 4.1 The anatomy of a call: authentication

Every AWS API call is an HTTPS request carrying a cryptographic signature (SigV4): the caller takes the request — method, URL, headers, body, timestamp — and signs it with a **secret key**. AWS looks up which identity owns the corresponding access key, re-computes the signature, and if it matches, knows two things: *who* is calling, and that the request wasn't tampered with — and because the timestamp is inside the signature, a captured request goes stale within minutes rather than being replayable forever. No cookies, no sessions, no "logged in" — each call independently proves its origin. (You never see this because the SDK and CLI do it for you, but it's happening on every call, including every click in the console.)

So authentication reduces to: **who holds a secret, and which identity does that secret map to?** The identities that can hold secrets are called **principals**. And here is where the design gets interesting, because there are two ways to be a principal, and the difference between them is the entire point of roles.

### 4.2 Users: the original sin

An **IAM user** is a principal with *long-lived* credentials — an access key pair that works until someone deletes it. This is the obvious design, the one you'd build first. It is also, in operational reality, the source of a huge fraction of cloud breaches, for reasons that have nothing to do with any AWS bug:

- A long-lived key is a *bearer* secret: anyone who has it, is you. It doesn't expire, doesn't know where it's being used from, and works from anywhere on the internet.
- Long-lived secrets *spread*. They get pasted into CI settings, baked into AMIs, committed to Git (there are bots scanning public GitHub for AWS keys; a leaked key is typically exploited in minutes), copied to a laptop that gets stolen.
- Rotation is a chore, so it doesn't happen. The key created in 2019 for a quick script is still valid, still admin, and nobody remembers it exists.

The industry lesson, learned expensively: **the problem isn't protecting secrets badly, it's having long-lived secrets at all.** Which sets up the actual design question behind roles: *can a principal exist that has no permanent secret whatsoever?*

### 4.3 Roles: identity without a secret

An **IAM role** is exactly that: a full principal — it has a name (an ARN), it has permissions — but **no credentials**. Nothing to leak, because there is nothing to hold. You cannot "log in as" a role.

Instead, a role is **assumed**. Some *other*, already-authenticated principal calls the STS API (`sts:AssumeRole`), and if permitted, receives back a set of **temporary credentials** — an access key, secret, and session token that behave like user credentials but **expire automatically** (typically after an hour, configurable up to a bound). For the lifetime of that session, the caller *acts as* the role.

The metaphor that makes everything downstream click: **a role is a costume, not a person.** It hangs on a rack. Different actors can wear it; while worn, the audience (the policy engine) sees the costume, not the actor; and it dissolves off your shoulders after an hour, so a stolen costume is a bounded problem rather than a permanent identity theft.

Because a role is a costume, it needs answers to two *separate* questions, and AWS makes you write them as two *separate* policies. This split confuses everyone at first and is obvious forever after:

1. **The trust policy** — *who may put this costume on?* (Formally: which principals may call `AssumeRole` on it.) It lives on the role, and it's what the console calls "trust relationships." Examples: "the EC2 service, on behalf of instances," "the CI pipeline's OIDC identity, but only from repo X on branch main," "account 1234's admin role, with MFA."
2. **The permission policy** — *what can whoever is wearing it do?* ("Read these S3 buckets, write that DynamoDB table, nothing else.")

Two doors, both of which must open. Most real-world "why is my role not working" pain is fixing one door while the other stays locked.

### 4.4 The magic trick: how code gets credentials with no secret anywhere

Now the payoff — the machinery that makes roles the load-bearing primitive of the entire cloud, and the part that answers "why are things the way they are" most directly.

Question: your application runs on an EC2 instance and needs to read S3. Where do its credentials come from? The pre-role answers were all terrible: bake keys into the image (leak), pass them via environment config (leak, no rotation), a secrets file (same). The role-based answer is genuinely elegant:

1. You attach a role to the instance (via an "instance profile").
2. The instance has a link-local **metadata service** — an HTTP endpoint at `169.254.169.254`, reachable *only from the instance itself* (it's answered by the virtualization substrate, not by any server on a network).
3. When code on the instance asks the metadata service for credentials, the substrate — which *knows* which instance is asking, because it's underneath it — calls STS on the instance's behalf and hands back temporary credentials for the attached role.
4. The SDK does this automatically. Your code contains **zero** secrets. Credentials appear from the environment, are short-lived, auto-renew, and never exist anywhere a developer could commit them.

Sit with what happened there: **authentication was derived from *where the code is running* — a fact the platform can verify directly — rather than from a secret the code holds.** That is the deep idea. Identity flows from verified context, not from possession of a string. Once you see this pattern, you'll recognise it everywhere, because the entire modern cloud is this trick repeated at different layers:

- **Lambda execution roles** — same trick; the function's runtime environment is injected with temporary credentials for its role.
- **ECS task roles / EKS IRSA / Pod Identity** — same trick at pod granularity: the orchestrator vouches for which pod is asking (in IRSA's case, by giving the pod a signed OIDC token that a role's trust policy accepts), so *each service in a cluster* gets its own narrowly-scoped identity, with no secret in any container image. This is exactly the mechanism behind pod-level identity with local secrets — now you know what it's made of.
- **CI federation (OIDC)** — same trick across organisational boundaries. GitHub Actions signs a token saying "this job is repo R, branch B"; a role's trust policy says "I accept tokens from GitHub's issuer for exactly repo R, branch B"; the pipeline assumes the role. Result: your CI deploys to AWS with **no stored AWS keys at all** — the leaked-CI-secret disaster class is deleted, not mitigated.
- **Human SSO** — same trick for people. Nobody should be an IAM user; humans authenticate to an identity provider (Entra, Okta, IAM Identity Center) and *federate into roles*, receiving temporary credentials per session. MFA lives at the IdP, offboarding is deleting one identity, and there are no permanent human keys to steal.
- **Cross-account access** — same trick between accounts: a role in account B trusts a principal in account A. Nobody ever "shares credentials" across accounts; they grant assumption rights, which are revocable, auditable, and expire per-session. This — plus every assumption being logged in CloudTrail — is what makes a hub-and-spoke topology governable.

**This is why the role is the way it is.** It looks over-engineered ("why can't I just have a password for my server?") until you see that it's the only known answer to *"how do millions of workloads authenticate, at scale, without a single long-lived secret in the system?"* The strange shape — no credentials, two policies, a token service in the middle — is the minimal machine that does that job.

### 4.5 Authorization: how "is this allowed?" is actually decided

Authentication established *who*. Authorization decides *whether*. The engine evaluates every call against every policy in scope, with three rules that are worth memorising because they resolve ~90% of IAM confusion:

1. **Default deny.** No policy says yes → the answer is no. (§3's inversion, applied.)
2. **Any explicit deny wins.** Over any number of allows, always. Denies are for guardrails ("nobody may disable CloudTrail, *even admins*"), and they cannot be argued with from below.
3. **Otherwise, one relevant allow suffices.**

What makes it *feel* complicated is that several policy types participate in one decision, but they're not redundant — each answers a different organisational question. **Identity policies** (attached to the user/role: "what can this principal do?") are the everyday ones. **Resource policies** (attached to the thing itself — an S3 bucket, a KMS key: "who may touch *me*?") let a resource speak for itself, which is what makes cross-account sharing sane: within one account, either side saying yes is usually enough (KMS is the notable exception — a key's own policy must explicitly delegate to IAM for identity-policy grants to count); **across accounts, both sides must say yes** — the caller's account must allow the call *and* the resource must accept the caller. **Permission boundaries** cap what an identity policy can grant — "this team may create roles, but no role they create can exceed *this* envelope" — which is how you delegate IAM itself without delegating escalation. **Service Control Policies (SCPs)** are the same idea one level up: organisation-wide ceilings applied to whole accounts, capable of binding even that account's root ("tenant accounts cannot leave the org, cannot disable audit logging"). Boundaries and SCPs *never grant* — they only shrink the maximum — which is exactly the property a ceiling needs.

> **Layered mental model:** SCP (org ceiling) ∩ permission boundary (delegation ceiling) ∩ identity/resource policies (actual grants) − explicit denies = what happens. Each layer exists because a different *level of the organisation* needs to say no: the org to its accounts, the platform team to its delegates, the engineer to their workload.

### 4.6 The same problem in three dialects

Azure and GCP face the identical problem and land on recognisably the same machine with different vocabulary — proof that this shape is forced, not fashionable:

| Concept | AWS | Azure | GCP |
|---|---|---|---|
| Workload identity, no stored secret | IAM Role (+ instance profile / IRSA) | Managed Identity | Service Account (attached to resource) |
| Human identity | Federated via IAM Identity Center / IdP | Entra ID (native — its great strength) | Google identity via Cloud Identity |
| Permission attachment | Policies attached to principals *and* resources | **Role assignments at a scope** (mgmt group → subscription → resource group → resource), inherited downward | **Bindings attached to resources**: (member, role) pairs on the resource hierarchy, inherited downward |
| Org container | Account (hard wall) / Organizations | Subscription / Management groups | Project (hard-ish wall) / Folders / Org |
| Temporary credentials | STS | Entra token issuance | Short-lived OAuth tokens |

The one structural difference worth internalising: **AWS reasons from the principal** ("what can this identity do?" — policies travel with the identity), while **GCP and Azure reason from the resource/scope** ("who can touch this thing/level?" — grants live on the object or the hierarchy and flow down). Same algebra, transposed. If you can answer "who can read this bucket, and via which path?" in all three dialects, you understand cloud identity — full stop.

### 4.7 Self-test for this section

You have the mental model when you can answer these cold, in your own words: *Why does a role have two policies instead of one? Where, physically, do an EC2 instance's credentials come from, and why can't another machine get them? Why does cross-account access require an allow on both sides, but same-account access doesn't? Why can't an SCP ever grant a permission? Why is "our CI has no AWS keys stored anywhere" not only possible but the correct default?* If any of these produce a pause, re-read the matching subsection — these five cover the whole load-bearing structure.

---

## 5. Networking: hardware nostalgia, implemented in software

The VPC confuses people for an interesting reason: it is a *simulation of something that no longer exists*. In a physical datacenter you had real private networks — your switches, your cables, your firewall at the edge. On a multi-tenant substrate none of that is physically yours, but customers needed the same guarantees (and the same mental furniture), so AWS rebuilt the private network *as software*: every packet's journey — which "subnet" it's in, what the "route table" says, whether the "security group" admits it — is decided by the Nitro cards and the network fabric consulting *data structures*, not by cables. A VPC is a database pretending to be a network. Once you see that, the pieces stop being mysterious:

- **VPC** — your private address space (a CIDR block, e.g. `10.0.0.0/16`), invisible to every other tenant even when their packets share the same physical wires (traffic is encapsulated and tagged with your VPC's identity; crossing tenant boundaries is impossible by construction, not by firewall rule).
- **Subnets** — subdivisions of that space, each pinned to one AZ (there's §1 again: subnets exist so *you* control which failure domain each thing lands in). "Public" vs "private" subnet is not a checkbox — it is purely *whether the subnet's route table has a route to an internet gateway*.
- **Route tables** — "where do packets addressed to X go next?" The entire topology — what's internet-facing, what's isolated, what goes through an inspection appliance — is these few lines of data.
- **Security groups** — per-resource virtual firewalls, and they're **stateful**: allow the inbound request and the response is automatically allowed back. Their most elegant feature: rules can reference *other security groups* ("allow 5432 from anything wearing the `app-tier` SG"), so you express *intent* ("app talks to DB") instead of chasing IP addresses. NACLs are the stateless, subnet-level, blunter sibling — most teams leave them default and do everything in SGs.
- **Internet Gateway / NAT Gateway** — the doors. IGW: two-way door for things with public IPs. NAT: one-way door letting private things *out* (for updates, APIs) while remaining unreachable from outside. NAT gateways charge per hour *and per GB processed* — which is why they quietly become a top-ten line on real bills (often a good chunk of the "EC2-Other" mystery on a bill). The cheap trick everyone learns late: traffic from private subnets to S3/DynamoDB can bypass NAT entirely via free **gateway endpoints** — same data, zero processing fee.
- **VPC endpoints / PrivateLink** — doors that lead to *services* rather than the internet: talk to S3, ECR, STS, or another team's service without your traffic ever touching public address space. The endgame of this machinery is a posture where "the internet" is simply not on the path for anything internal.

> **Mental model to keep:** deny-by-default (§3) applied to packets, over a software-defined simulation of 1990s networking. Nothing can reach anything until a route *and* a rule say so. And every hop that does exist is a line item — when a packet's path costs money (NAT, cross-AZ, egress), the bill is telling you about your architecture.

---

## 6. Compute: a spectrum of surrendered control

Every compute option on every cloud sits on one axis: **how much of the machine are you still willing to think about?** Each step rightward hands the provider more of the operational problem, in exchange for less control and (often) a higher price per raw unit — but a lower *total* cost once you count the ops work you stopped doing.

```
more control ◄──────────────────────────────────────────────► more surrender
EC2 (VM)      →  containers you    →  Fargate (containers,  →  Lambda (functions,
your OS,         schedule (ECS/EKS     no nodes at all)         no processes at all)
your runtime,    on your nodes)
your problem
```

- **EC2**: a slice of a physical machine (§3). You own the OS upward: patching, capacity, failure replacement. Maximum freedom, maximum toil.
- **Containers on EC2 (EKS/ECS)**: you've standardised the unit of deployment (the image — one artefact, built once, bit-identical everywhere), and a scheduler places them. But you still run the node fleet under them.
- **Fargate**: you hand over the nodes; you say "run this container with this CPU/memory" and there is no instance to patch. You pay a premium per vCPU-hour for the privilege of not owning nodes.
- **Lambda**: you hand over *everything but the function*. Scale-to-zero, per-millisecond billing, massive parallelism for free; in exchange: time limits, cold starts, size limits, and a programming model you must design around.

Two ideas to attach here, because they explain most compute-adjacent AWS behaviour:

**The shared-responsibility line is what actually moves.** "Managed service" means precisely: the line between what they operate and what you operate has shifted upward. RDS is Postgres with the OS, replication, backups, and failover moved to their side of the line — you still own schema, queries, and capacity choice. Every AWS service page is answering one question: *where's the line?*

**Cattle, not pets — and it's forced, not fashionable.** At datacenter scale, hardware fails constantly as a statistical certainty, so the provider's contract is "instances are ephemeral; design for replacement, not repair." This single assumption generates: autoscaling groups (declare a desired count; the system converges), immutable AMIs/images (replace, don't patch in place), load balancers as the stable name in front of unstable members, and — one level up — Kubernetes, whose entire philosophy (declare desired state; controllers reconcile reality toward it, forever) is this idea industrialised. Note where this leads: **GitOps and fleet-management systems are the same idea again, one more level up** — declared state + reconciliation loop, applied to whole fleets of clusters. It's reconciliation all the way down.

---

## 7. Storage and state: where the gravity is

Compute is trivially elastic — instances are interchangeable and disposable. **State is the opposite**: data has integrity requirements, durability requirements, and *mass* (moving it takes time and money). Every hard problem in cloud architecture is ultimately a state problem, and every storage product is a different answer to "what shape is your state, and what do you need to be true about it?"

- **Block storage (EBS)** — a virtual disk: raw blocks attached to one instance, in one AZ. It's the local-disk illusion for things that need a filesystem (databases, mostly). AZ-bound and (barring a niche multi-attach feature) single-writer — the least "cloudy" storage, kept because databases need it.
- **Object storage (S3)** — the cloud-native answer, and the true centre of gravity of AWS. Not a filesystem: a flat key→blob map over HTTP, no partial updates, no locks. Those constraints (immutability, no random writes) are exactly what let AWS replicate and erasure-code every object across ≥3 AZs and honestly promise **eleven nines of durability** — at which point S3 stops being "a place for files" and becomes *the substrate everyone builds on*: data lakes, backups, log archives, static sites, ML artefacts, deployment bundles. Rule of thumb: if the data doesn't need to be a live disk or a queryable database *right now*, it belongs in S3, in the cheapest storage class its access pattern allows (Standard → IA → Glacier tiers — the lifecycle-transition lever).
- **Managed databases** — the shared-responsibility line applied to state: RDS/Aurora (relational, their ops), DynamoDB (AWS's own key-value answer to "what if the database itself were built cattle-not-pets?" — partitioned, replicated, no instances visible at all, in exchange for a constrained query model), Redshift (columnar, for scanning billions of rows, not for transactions), ElastiCache, and a long tail. The proliferation isn't product spam — under multi-tenant economics, each access pattern genuinely optimises differently, and the provider can afford to build one service per pattern.

**And state is where the money and the lock-in live.** Storing data is cheap; *moving* it is not. Ingress is free; **egress to the internet costs real money** (order of ~$0.09/GB) — cross-region and even cross-AZ transfer are billed too. Read those prices as strategy, not just engineering: data is easy to bring, costly to remove, and every service that touches your data where it sits (Athena querying S3 in place, Redshift Spectrum, SageMaker) deepens the gravity well. This isn't a complaint — it's the honest shape of the deal, and knowing it is how you make lock-in a *decision* (often a fine one) rather than a surprise.

---

## 8. The economics: the pricing model *is* the architecture

You can read a cloud provider's entire engineering reality off its price list — pricing is where the physics and the business model become numbers. Four lenses:

**1. What business are they actually in?** Turning **capex into opex**, and *risk* into *margin*. You stop buying servers 18 months ahead of demand (and being wrong in both directions); they aggregate everyone's demand curves, which is statistically smoother than any single customer's, and charge a margin for absorbing the variance. Elasticity isn't a feature of the cloud — it *is* the product.

**2. Provisioned vs consumed.** Every service bills one of two ways, and the distinction should drive your choices: **provisioned** (EC2, RDS, Redshift, MSK, NAT hours — you pay for *existence*, whether used or not) vs **consumed** (Lambda, S3 requests, DynamoDB on-demand — you pay for *use*). Idle provisioned capacity is the single largest waste category in real bills (a dev cluster running overnight costs the same as one doing work), which is why "scale to zero," "pause when idle," and "schedule non-prod shutdowns" recur in every cost review.

**3. Committed vs on-demand vs spot: three prices for the same CPU.** On-demand is the expensive, zero-commitment price. **Savings Plans / Reserved Instances** (~30–70% off) are you selling your demand *predictability* back to the provider — it's a forward contract; they can plan capacity, you split the surplus. **Spot** (up to ~90% off) is their excess-inventory market: unused capacity, reclaimable at two minutes' notice — free money for interruptible work (batch, CI, stateless workers) and a trap for anything that can't die gracefully. A mature estate runs all three deliberately: commitments covering the steady floor, spot eating the batch, on-demand only for the genuinely unpredictable residual. On most real estates, Savings Plans are the single biggest lever.

**4. Prices are messages.** Free ingress + costly egress = "bring data, don't leave" (§7). NAT per-GB fees = "we'd rather you used endpoints." Cross-AZ transfer fees = "resilience has a real cost; we're passing it through." Lambda's per-ms pricing = "we can afford fine-grained billing because we bin-pack you with a million others." When a price seems weird, ask what behaviour it's incentivising — there's almost always a real cost or a real strategy underneath.

---

## 9. The provider landscape: same physics, three accents

Spend your depth on AWS — but hold the model at one level of abstraction up, because **all three major providers converge on the same shape** (regions/AZs, deny-by-default identity, software-defined networks, object storage at the centre, a compute-surrender spectrum, commitment pricing). They converge because they face the same physics, the same security problem, and the same economics. What differs is *accent* — each provider's origin story showing through:

- **AWS — primitives first.** Grown from Amazon's internal platform (§2): a huge catalogue of sharp, composable, sometimes overlapping building blocks, with integration left as an exercise for the customer. Deepest catalogue, biggest ecosystem and talent pool, most operational maturity; the default choice, and the default *syllabus* — AWS concepts map outward better than the reverse.
- **Azure — the enterprise agreement, continued.** Microsoft's cloud sells to the CIO: seamless with Entra ID/AD, Office, Windows licensing, and existing Microsoft contracts. Identity is genuinely its crown jewel (Entra is the identity layer for most large enterprises *anyway*). Hierarchy-heavy governance (management groups → subscriptions → resource groups) that mirrors how big organisations think.
- **GCP — the engineers' cloud.** Google externalising its internal infrastructure: the strongest data/analytics products (BigQuery is the best single service on any cloud, and often the whole reason a company is on GCP), Kubernetes born here and GKE still the reference implementation, a genuinely global network (one VPC can span regions — they let their private-fibre advantage show). Smaller catalogue, opinionated, historically weaker enterprise sales muscle.

A working rosetta stone for the concepts you'll actually reach for:

| | AWS | Azure | GCP |
|---|---|---|---|
| Hard org boundary | Account | Subscription | Project |
| Grouping / governance | Organizations + OUs | Management groups + resource groups | Organization + folders |
| VM | EC2 | Virtual Machines | Compute Engine |
| Object storage | S3 | Blob Storage | Cloud Storage |
| Managed Kubernetes | EKS | AKS | GKE |
| Functions | Lambda | Azure Functions | Cloud Functions / Cloud Run |
| Workload identity | Role + instance profile/IRSA | Managed Identity | Service Account |
| Org-wide guardrail | SCP | Azure Policy | Org Policy |
| Data warehouse | Redshift | Synapse / Fabric | BigQuery |
| IaC-native tool | CloudFormation/CDK | ARM/Bicep | Infrastructure Manager (in practice: Terraform everywhere) |

The practical strategy this table implies: learn AWS deeply, learn the *mapping* shallowly, and treat any "which cloud?" question as a question about accents (existing Microsoft estate → Azure gravity; analytics-centred → GCP gravity; everything else → AWS default) rather than about fundamentals — the fundamentals are the same machine.

---

# Part II — The model, compressed: seven forces that explain everything

If Part I did its job, you can now derive most of AWS rather than memorise it. Here is the whole thing folded down to seven principles. When you meet an unfamiliar service or a weird-looking constraint, run down this list — one of these is almost always the explanation.

1. **Everything is an API; therefore identity is the perimeter.** No inside, no trusted network. Every capability is an authenticated call; every call is policy-checked; the console is just a client. (§2, §4)
2. **Deny by default, everywhere.** Multi-tenancy makes default-open unsurvivable, so identity, networking, and cross-account access all start at "no" and require written permission to become "yes." Fiddliness is the feature. (§3, §4.5, §5)
3. **No long-lived secrets; identity flows from verified context.** The role/STS/metadata machinery exists to replace possession-of-a-string with proof-of-where-you're-running. Anywhere you still hold a permanent credential, you're using the cloud against its grain. (§4.3–4.4)
4. **Blast radius is managed by nesting failure and trust domains.** AZ ⊂ region ⊂ planet for failures; role ⊂ account ⊂ OU ⊂ organization for trust. Every resilience feature and every governance feature is a statement about one of these boundaries. (§1, §3, §4.5)
5. **Control plane and data plane are separated, so that change can fail without work failing.** Provider services are built this way, and systems you build on top should be too. (§2)
6. **Declared state + reconciliation beats imperative commands.** Autoscaling groups, Kubernetes, GitOps, a fleet reconciler: say what should be true, let a loop make it true, continuously. The imperative alternative cannot survive scale or drift. (§6)
7. **The pricing model is the architecture, restated in dollars.** Existence vs use, commitment vs flexibility, data-in vs data-out. Read bills as architectural documents. (§7–8)

Worth noticing: none of these is AWS-specific, and only #7 is even cloud-specific in spirit. They're what *any* planet-scale shared computer would converge on — which is why all three providers have.

---

# Part III — The model, applied: reading your own estate through it

This section comes last — but it's worth doing deliberately, because a mature estate has usually already *built* with these principles by instinct. Naming which principle each existing decision instantiates is the fastest way to make the model stick.

- **Account-per-tenant** is Principle 4 played at maximum strength: the account is the strongest nested boundary available, used as the tenant blast-radius/trust/quota/billing wall simultaneously. "The account boundary is the whole game" is this guide's §3, applied.
- **Hub-and-spoke with a pull-based reconciler** is Principles 3, 5, and 6 composed: OIDC federation instead of stored CI keys (3), a control plane whose outage stops change but never work (5), and per-tenant reconcilers converging on declared state (6). The "no credential convergence" test is Principle 3 stated as an organisational rule.
- **IRSA / pod identity / tenant-local secrets** is §4.4's magic trick, deployed. Where it once read as "the incantation that makes security people happy," it should now read as: identity derived from verified runtime context, so that no secret exists to leak.
- **Waves, gates, and N-1 contracts** are an application of Principle 4 — rollout *blast-radius nesting* (canary tenant ⊂ wave ⊂ fleet) layered on top of AWS's own, plus Principle 6's observed-not-asserted state (`current` written only by telemetry).
- **A cost review** is Principle 7 as a workbook: EC2-Other decoded into NAT/EBS plumbing (§5's "every hop is a line item"), Savings Plans as selling predictability (§8.3), idle-provisioned waste (§8.2), lifecycle transitions as storage-class economics (§7). Read one after this guide — every recommendation should look like an instance of a principle rather than a tip.

The satisfying conclusion: a well-built fleet-management design and AWS itself are *the same design* at different scales — declared state, reconcilers, nested blast radii, no credential convergence, control/data separation. Nobody copied AWS; the same constraints forced the same answer. That's the strongest possible evidence the mental model is real.

---

# Part IV — The structured path: from model to mastery

The model above is scaffolding; it becomes *yours* through deliberate contact with the real thing. This path is sequenced so each stage cements one part of the guide, with a concrete exercise (do them in a personal sandbox account — total cost across everything here is a few pounds, less if you stay in free tier) and a checkpoint question you should be able to answer *from memory, in writing* before moving on. Roughly a stage every week or two alongside a job; the order matters more than the pace.

**Stage 1 — Touch the API directly (cements §2).**
Install the AWS CLI. Create a bucket, list it, put an object, delete it — CLI only, no console. Then run the same commands with `--debug` once and *look at* the signed request going out.
✅ *Checkpoint:* explain to a rubber duck why "the console is just an API client" changes what IaC is.

**Stage 2 — Assume a role by hand (cements §4 — the IAM deep dive).**
This is the highest-value exercise in the whole path. In your sandbox: (1) create a role with a trust policy allowing your own user to assume it, and a permission policy allowing S3 read only; (2) call `aws sts assume-role` yourself; (3) export the three returned values (key, secret, *session token*) and make calls as the role; (4) watch a call fail outside the permission policy; (5) wait for expiry and watch the credentials die; (6) find the assumption event in CloudTrail. Then break it both ways: wrong trust policy (can't assume), wrong permission policy (assume but can't act) — feel the two doors from §4.3 separately.
✅ *Checkpoint:* narrate, without notes, exactly where an EC2 instance's credentials come from and why leaking them is time-bounded.

**Stage 3 — Build the classic network (cements §5).**
From scratch, by hand, once in your life: VPC → two subnets (public/private, different AZs) → IGW → route tables → an instance in each → NAT for the private one → security group referencing another security group. Break it deliberately: delete the IGW route, watch "public" become meaningless.
✅ *Checkpoint:* explain what makes a subnet "public" (precisely), and why NAT data processing shows up as a cost line but a gateway endpoint doesn't.

**Stage 4 — Feel the compute spectrum (cements §6).**
Deploy the same trivial HTTP service four ways: on an EC2 instance by hand; as a container on ECS/Fargate; as a Lambda behind a function URL. (Skip EKS here if you already use it daily.) For each, write one line: *what did I stop having to think about, and what control did I lose?*
✅ *Checkpoint:* given a random workload description, place it on the spectrum and defend the placement in cost + ops terms.

**Stage 5 — Everything as code (cements §2 + §6 together).**
Rebuild Stage 3's network in Terraform. Then change something in the console by hand and run `terraform plan` — watch drift detection see it. This is the declared-state/reconciliation loop (Principle 6) in your hands, and the direct ancestor of GitOps machinery for whole fleets.
✅ *Checkpoint:* explain why IaC is possible at all (§2), and what a non-empty plan on an unchanged config *means*.

**Stage 6 — Read architectures like literature (consolidation).**
Now switch from building to reading, with the seven principles as your critical lens: the **AWS Well-Architected Framework** (the six pillars are AWS's own compression of this guide — you'll recognise everything); a few entries from the **AWS Architecture Center / This Is My Architecture**; and one or two post-incident writeups of major AWS outages (the us-east-1 ones are instructive precisely about control/data-plane separation and blast radius). For each: *which principle is doing the work in this design? which boundary failed in this outage?*
✅ *Checkpoint:* take any architecture diagram at work and annotate it with which of the seven principles each component embodies — and where one is being violated.

**Stage 7 — The other dialects (cements §9).**
Two weekends, not two months: on GCP, create a project, a service account, a bucket, and a binding; on Azure, a resource group, a managed identity, and a role assignment at a scope. The goal is purely to *feel* the principal-centric vs resource-centric transposition from §4.6.
✅ *Checkpoint:* answer "who can read this bucket?" in all three dialects.

**Ongoing — the drip feed.** A small, high-signal diet beats courses: **Last Week in AWS** (Corey Quinn — cost and services commentary with the marketing stripped off), a couple of re:Invent deep-dives a year (anything by **Colm MacCárthaigh** on how AWS builds reliable systems, and **James Hamilton's** infrastructure talks for the physical layer of §1 — both searchable on YouTube), and the **Open Guide to AWS** (GitHub) as the honest field manual. If you want a forcing function with a syllabus, the **Solutions Architect Associate** certification's curriculum (e.g. Adrian Cantrill's course, widely considered the best-taught) covers Stages 1–5 systematically — treat the cert as optional, the syllabus as the value.

**The finish line** isn't knowing every service — nobody does, and the catalogue is deliberately larger than any one architecture needs. It's this: when you meet an unfamiliar AWS feature, you can predict *why it exists and roughly how it must work* before reading its documentation — because there are only seven forces, and you know all of them.

---



