# Phase 4 — Operations

**When:** Ongoing, starting as soon as there's anything worth losing.

## Goal

Three habits that are worth more than any additional service, and are most of what
separates a junior from a senior in practice.

## 1. Backups you have actually restored

**An untested backup is a rumour.**

- Automate the backup. Database dump plus anything on disk that isn't in git.
- Store it somewhere the primary system can't delete (different provider or S3 with
  versioning and a lifecycle policy).
- **Restore it.** Spin up a fresh box, restore into it, and confirm the data is
  really there and the app runs against it.
- Repeat the restore once a quarter. Put it in the calendar.

The restore is the whole exercise. Backup scripts that have never been restored fail
at roughly the rate you'd fear.

## 2. Monitoring that would actually wake you up

The question to answer is: **would I find out my site was down before a user told
me?**

Start with something hosted on a free tier — Grafana Cloud, Better Stack, or even
just an uptime pinger. Self-hosting Prometheus and Grafana on day one means you now
have two systems to keep alive and the monitoring goes down with everything else.

Minimum useful set:

- An external uptime check hitting a real URL (not just "is the port open")
- Alerts to somewhere you'll actually see them
- Disk space warnings — full disks are the most boring and most common outage
- Certificate expiry warnings, unless fully automated
- Somewhere to read logs without SSHing in

Graduate to self-hosted Prometheus later, deliberately, when you want to understand
it.

## 3. A runbook per project

One markdown file. Write it while things are calm, because you won't write it at 2am.

Contents:

- How to deploy
- **How to roll back**
- Where the logs are
- Where the backups are and how to restore one
- What to check first when it's down
- Who else needs to know
- Every credential's location (not the credential — its location)

## Done when

These are habits, not milestones, but a reasonable bar:

- [ ] A backup runs automatically without you thinking about it
- [ ] You have restored from that backup onto a fresh machine at least once
- [ ] An alert reaches you when the site goes down, and you have tested it by
      deliberately breaking something
- [ ] A runbook exists for each running project
- [ ] You know how much disk is free on every box you own, right now

## Notes
