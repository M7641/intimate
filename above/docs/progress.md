# Progress

Started 2026-09-06. Tick things off as they're genuinely done — "done" means you
could do it again unaided, not that you read about it.

## Phase 0 — Guardrails ([doc](01-guardrails.md))

**AWS account**
- [ ] Account created
- [ ] MFA device #1 on root
- [ ] MFA device #2 on root (no backup codes exist — this is the backup)
- [ ] Signed out and back in as root to confirm MFA is enforced

**Daily-use identity**
- [ ] IAM user `mike` created with AdministratorAccess
- [ ] Account alias set (nicer sign-in URL)
- [ ] MFA on the IAM user
- [ ] Signed in as `mike` successfully; root retired

**Billing**
- [ ] IAM access to Billing activated (root-only toggle — easy to miss)
- [ ] CloudWatch billing alerts + Free Tier alerts enabled in Billing preferences
- [ ] Budget 1: `learning-early-warning` at 10, alerts at 80% actual + 100% forecast
- [ ] Budget 2: `learning-backstop` at 30, alert at 100% actual
- [ ] Confirmation email received
- [ ] Checked back after 24h and the budget shows real spend data

**CLI**
- [ ] `aws-cli-v2` installed
- [ ] Access key created and `aws configure` run (region `eu-west-2`)
- [ ] `~/.aws/credentials` is chmod 600
- [ ] `aws sts get-caller-identity` returns the IAM user
- [ ] `aws iam get-account-summary` shows `AccountMFAEnabled: 1`
- [ ] `aws budgets describe-budgets` lists both budgets

**Hetzner**
- [ ] Account created and identity verification passed
- [ ] Project `lab` created
- [ ] Ed25519 SSH key generated and uploaded

**Domain**
- [ ] Domain purchased
- [ ] Can log in to the registrar and edit DNS records

## Phase 1 — One server ([doc](02-first-server.md))

- [ ] Server provisioned
- [ ] Non-root sudo user, key-only SSH
- [ ] Password auth and root login disabled
- [ ] Firewall configured and understood
- [ ] Unattended upgrades on
- [ ] DNS pointed at the box (A + AAAA)
- [ ] Caddy serving HTTPS on the real domain
- [ ] App running as a systemd unit, as a non-root user
- [ ] Survives reboot
- [ ] Restarts after `kill -9`
- [ ] Rebuilt from scratch — 1st time (by hand)
- [ ] Rebuilt from scratch — 2nd time (writing steps down)
- [ ] Rebuilt from scratch — 3rd time (as a script)
- [ ] Read a week of SSH logs

## Phase 2 — Reproducible ([doc](03-reproducible.md))

- [ ] App containerised
- [ ] Images pushed to ghcr.io
- [ ] GitHub Actions builds on merge to main
- [ ] Actions deploys automatically
- [ ] Can identify the running commit in production
- [ ] Second server provisioned
- [ ] Load balancer in front of both
- [ ] One server can be taken down without an outage
- [ ] Session state question answered and written down
- [ ] File upload question answered and written down
- [ ] Database moved off the app servers

## Phase 3 — AWS + Terraform ([doc](04-aws-terraform.md))

- [ ] Terraform state in versioned S3 bucket
- [ ] VPC with public + private subnets across 2 AZs
- [ ] Internet gateway + NAT gateway
- [ ] EC2 in a private subnet, no public IP
- [ ] ALB in public subnets, health checks passing
- [ ] ACM certificate, HTTPS at the real domain
- [ ] RDS Postgres in private subnets
- [ ] S3 bucket for uploads
- [ ] IAM role on the instance — zero access keys anywhere
- [ ] Full `destroy` → `apply` rebuild works unaided
- [ ] Read the Cost Explorer and understood the bill

## Phase 4 — Operations ([doc](05-operations.md))

- [ ] Automated backup running
- [ ] Backup restored onto a fresh machine — date: ______
- [ ] Backup restored again (quarterly) — date: ______
- [ ] External uptime check
- [ ] Alerts reach me somewhere I'll see them
- [ ] Alert tested by deliberately breaking something
- [ ] Disk space alerting
- [ ] Runbook written per project

## Log

Dated notes — what was learned, what broke, what surprised you. The most valuable
file here in six months.

### 2026-09-06
Plan written. Nothing built yet.
