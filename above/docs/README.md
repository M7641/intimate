# Infrastructure Learning Journey

A self-directed path from "comfortable on my own machine" to "can design, run and
debug real systems in the cloud."

Started: 2026-09-06

## Who this is for

Written for my own situation, and the plan is shaped by it:

| | |
|---|---|
| **Goal** | All three, equally: employability, shipping my own projects, and genuinely understanding how it works |
| **Starting point** | Comfortable with Linux locally (Arch daily driver), shaky on servers exposed to the internet |
| **Budget** | £20-50/month |
| **Location** | UK |

The gap that matters most is the third row: everything that changes when a machine
has a public IP and strangers can reach it. Most of the early plan attacks that
directly.

## The core idea

There are two separate skills that get confused with each other:

- **The transferable layer** — Linux, networking, TLS, process supervision, storage,
  observability, backup and restore. Takes years. This is what makes you a better
  engineer.
- **The vendor layer** — what AWS calls a VPC vs. what GCP calls a VPC, and 400 pages
  of console. Takes about three weeks once you have the first.

Big clouds are very good at letting you skip the first one. Click "deploy container",
it works, you learned nothing. This plan deliberately does the fundamentals on a bare
VPS first, then layers the vendor vocabulary on top.

See [00-cloud-choice.md](00-cloud-choice.md) for why AWS + Hetzner rather than
anything else.

## The sequence

| Phase | When | What | Doc |
|---|---|---|---|
| 0 | Week 1 | Guardrails — billing alarms, MFA, a domain | [01-guardrails.md](01-guardrails.md) |
| 1 | Weeks 1-4 | One server, done properly, by hand | [02-first-server.md](02-first-server.md) |
| 2 | Weeks 4-8 | Make it reproducible — containers, CI/CD, two boxes | [03-reproducible.md](03-reproducible.md) |
| 3 | Weeks 8-16 | AWS, via Terraform only | [04-aws-terraform.md](04-aws-terraform.md) |
| 4 | Ongoing | Backups, monitoring, runbooks | [05-operations.md](05-operations.md) |
| — | Later | Deliberately deferred: Kubernetes, certs, serverless | [06-not-yet.md](06-not-yet.md) |

Track progress in [progress.md](progress.md).

## Running costs

| Item | Cost |
|---|---|
| Hetzner CX22 (primary box) | ~€4/mo |
| Hetzner CX22 (second box, phase 2) | ~€4/mo |
| Domain | ~£10/yr (~£1/mo) |
| AWS, run in bursts | £10-25/mo depending on NAT gateway uptime |
| **Total** | **~£20-35/mo** |

The AWS number is the volatile one. A NAT gateway is roughly $32/month if left
running, which is most of the budget on its own — so stand the environment up, work
on it, and `terraform destroy` when done. See
[04-aws-terraform.md](04-aws-terraform.md#cost-discipline).

## How to use these docs

Each phase doc has a **Goal**, the **work**, and a **Done when** section. The "done
when" is the bit that matters — it's what you should be able to do unaided, not what
you've read about.
