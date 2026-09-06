# Phase 3 — AWS, via Terraform only

**When:** Weeks 8-16.
**Cost:** £10-25/month if run in bursts. See [cost discipline](#cost-discipline).

## Goal

Learn the vocabulary the industry speaks, and build the environment that reads as CV
signal. By now the fundamentals are in place, so AWS is a mapping exercise rather
than magic.

## The one rule

**If you want a resource to exist tomorrow, Terraform creates it — not the console.**

Use the console to *look* at things. Never to *make* them.

Breaking this rule is the single most common way people spend a year on AWS and come
out unable to rebuild anything. Clicked resources are invisible, undocumented,
unreviewable and unreproducible, and they're what generate surprise bills, because
nobody remembers they exist.

Keep Terraform state in an S3 bucket with versioning on, not on your laptop.

## What to build

One realistic environment. This set covers most of what a genuine production
footprint uses:

| Resource | What it teaches |
|---|---|
| **VPC** with public + private subnets across 2 AZs | Subnetting, route tables, why "private" means "no route to an internet gateway" |
| **Internet gateway + NAT gateway** | How private instances reach the internet outbound but aren't reachable inbound |
| **EC2** instance in a private subnet | That the box you learned in phase 1 is the same box, just wrapped |
| **Application Load Balancer** in public subnets | Target groups, health checks, TLS termination, why the ALB is the only public thing |
| **RDS Postgres** in private subnets | Managed database trade-offs, subnet groups, backup windows |
| **S3** bucket for uploads | Object vs. block storage, bucket policies, presigned URLs |
| **IAM roles** attached to the instance | Credential-free access. **Never paste access keys into config.** |
| **ACM certificate** on the ALB | Managed TLS, and DNS validation |
| **Route 53** or your existing registrar's DNS | Pointing the real domain at the ALB |

Build it in that order. Each layer depends on the one above.

## Lessons to expect

These are the ones everyone hits, so recognise them when they arrive:

- **Security groups vs. NACLs.** Security groups are stateful and attach to
  resources; NACLs are stateless and attach to subnets. Almost always you want
  security groups and should leave NACLs alone.
- **The NAT gateway costs more than everything else combined.** ~$32/month plus data
  processing. It is the main thing standing between you and a small bill.
- **IAM policies never do what you first expect.** Explicit deny beats allow;
  resource policies and identity policies are different things; the policy simulator
  is genuinely useful.
- **The first `terraform apply` that creates a VPC takes about 3 minutes. The one
  that creates RDS takes 15.** Slow feedback loops are part of the job.

## Cost discipline

Run this phase in bursts:

```
terraform apply    # stand it up, work on it
terraform destroy  # when you stop for the day
```

If `destroy` becomes painful — because it takes down your database, or because you
want the environment permanently — split the Terraform into a long-lived stack
(VPC, S3, state) and an expensive-and-disposable stack (NAT gateway, RDS, EC2).
That split is itself a real-world pattern.

Check the Cost Explorer weekly for the first month. Learning to read a cloud bill is
a legitimate infrastructure skill and almost nobody teaches it.

## Done when

- [ ] `terraform destroy` then `terraform apply` rebuilds the whole environment unaided
- [ ] Zero resources in the account were created by clicking
- [ ] The app is reachable over HTTPS at a real domain via the ALB
- [ ] The EC2 instance has no public IP and no access keys on it
- [ ] You can explain, from memory, why the NAT gateway is there and what it costs
- [ ] Terraform state lives in S3, not on the laptop

## Notes
