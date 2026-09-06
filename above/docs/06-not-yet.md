# Deliberately deferred

Things that look like the obvious next step and aren't — with the reason, so the
decision can be revisited honestly rather than out of avoidance.

## Kubernetes

**Why not yet:** it solves problems you don't have. Learning it before understanding
what it's orchestrating produces someone who can copy YAML but can't debug a pod that
won't schedule — a very common and very visible failure mode.

**When to pick it up:** once you've genuinely felt the pain of managing a dozen
containers across several machines by hand. At that point it'll take a fraction of
the time and actually make sense, because every concept maps to a problem you've
personally had.

**Where to start when the time comes:** k3s on the Hetzner boxes, not managed EKS.
Cheaper, and you'll see the control plane instead of it being hidden.

## Certifications

**Why not yet:** doing the building first means the cert takes a week instead of two
months, and you'll be learning names for things you already understand rather than
memorising trivia about services you've never used.

**When:** if a specific job application asks for one. AWS Solutions Architect
Associate is the default choice.

## Serverless (Lambda, API Gateway, etc.)

**Why not yet:** Lambda is genuinely good, but it hides exactly the layer that's the
current gap. "Comfortable locally, shaky on servers" doesn't get fixed by never
touching a server.

**When:** after phase 3. It's a much better tool once you know what it's replacing —
and there are real workloads (cron jobs, webhooks, glue) where it's clearly correct.

## Small clouds (Fly.io, Render, Railway)

**Why not yet as a primary:** pleasant and fast, but they abstract away the learning
target, and their vocabulary doesn't transfer to interviews or other teams' systems.

**When:** any time, as a *tool* for shipping something quickly, once the underlying
model is solid. The distinction is using them by choice rather than because the
alternative is opaque.

## Service meshes, GitOps, multi-region, autoscaling

**Why not yet:** all real, all solving problems of scale and organisation that a
single learner with two boxes does not have. Reaching for them early is the clearest
signal of someone who learned infrastructure from conference talks rather than from
running things.
