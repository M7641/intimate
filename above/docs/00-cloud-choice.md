# Cloud choice and rationale

The decision, and why — so future-me remembers the reasoning rather than just the
conclusion.

## Decision

**AWS as the primary cloud, with a cheap Hetzner VPS running alongside it.**

Not either/or. The two do different jobs:

- **Hetzner** is where fundamentals get learned with nothing hiding them. A bare
  Ubuntu VM and a public IP means DNS, TLS, reverse proxying, systemd, firewalls,
  backups and monitoring are all mine to do. That's where the actual engineering
  growth happens, and at €4/month there's no cost pressure discouraging experiments.
- **AWS** is where the industry's vocabulary gets learned, and where the thing worth
  putting on a CV gets built.

Either alone is worse. AWS alone teaches button-clicking. Hetzner alone leaves you
unable to talk to anyone else in the industry.

## The three majors, assessed honestly

### AWS — chosen

Roughly 30% market share, but the more important fact is that it sets the industry's
vocabulary. S3, IAM, EC2, VPC are near-generic terms now. Most job ads, most
postmortems worth reading, and most Stack Overflow answers assume AWS.

Against it: the console is genuinely bad, and the service catalogue is a graveyard of
overlapping, half-abandoned products. But the primitives underneath are solid, and
IAM — painful as it is — is the best authorisation model you'll be forced to learn.
Best free tier of the three, and the docs are dense but complete.

### GCP — good, not chosen

Cleaner design throughout. Networking is genuinely coherent (global VPCs, real load
balancers), IAM is simpler, the CLI is better, and BigQuery has no real peer. Smaller
market share means fewer job ads mention it, though the ones that do skew data-heavy
or Kubernetes-heavy.

Google's deprecation reputation is somewhat unfair at the infrastructure layer, but
not entirely.

### Azure — the one to reconsider later

Specifically relevant given a UK base: Azure has disproportionate share in UK
enterprise, public sector, NHS and finance. If the career target turns out to be that
world, Azure is a stronger bet than its global market share suggests.

The engineering experience is the weakest of the three — sprawling portal,
inconsistent naming, wildly variable docs. But "the technology is nicer" loses to
"this is what employers here actually run." Worth revisiting this decision if a job
search points that way.

## Why not a small cloud only?

Fly.io, Render, Railway and friends are genuinely pleasant and would ship projects
faster. They're skipped as a *primary* because they abstract away exactly the layer
that needs learning, and their vocabulary doesn't transfer to interviews or to other
teams' systems.

Worth using later, once the underlying model is solid — at that point they're a tool,
not a crutch.

## Revisit this decision when

- A concrete job target implies a different cloud (especially UK public sector → Azure)
- A project has a genuine data-warehouse shape (→ GCP/BigQuery is a real advantage)
- Twelve months have passed and the market has moved
