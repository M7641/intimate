# Phase 2 — Make it reproducible

**When:** Weeks 4-8.
**Cost:** a second Hetzner box, ~€4/month.

## Goal

Go from "I can build a server" to "a machine builds and deploys it for me", and then
discover what breaks when there's more than one server.

## Work

### 1. Containerise

Write a Dockerfile for the app. Run it on the box with Docker. Caddy now proxies to
the container instead of a systemd-managed binary.

Worth understanding while here: what a container image actually is (layers, a
filesystem, a process — not a VM), and why the process inside sees PID 1.

### 2. Registry

Push images to GitHub Container Registry (ghcr.io). Free, and it sits next to the
code.

### 3. CI/CD with GitHub Actions

On merge to `main`: build the image, tag it with the commit SHA, push it, then SSH to
the box and restart with the new tag.

**SSH-in-and-restart is a completely legitimate deploy mechanism at this scale.**
Pretending otherwise is how people end up running Kubernetes to serve a blog. The
valuable part is that deploys are now automatic, repeatable and tied to a commit —
not the sophistication of the mechanism.

Store the deploy SSH key as a GitHub Actions secret. Give it a dedicated, restricted
user on the box.

### 4. Add a second server

Provision another box, run the same container, put a load balancer in front (Hetzner
sells one, or run Caddy on a third box as the balancer).

**This is where the real lesson lands.** The moment there are two servers, state
becomes a problem you have to actually solve:

- Where do user sessions live, if a request can hit either box?
- Where do uploaded files go, if box A can't see box B's disk?
- How do both boxes reach one database?
- What happens during a deploy when one box has the new version and one has the old?

Answering those four questions on your own infrastructure teaches more about
distributed systems than any amount of reading. Write down the answers you land on.

### 5. Move the database off the app servers

If you haven't already, the database now needs to be somewhere both boxes can reach.
Either a dedicated Hetzner box you manage, or a managed Postgres. Doing it yourself
once is instructive; the answer to "should I run my own production database" is
usually no, but you should know *why* from experience.

## Done when

- [ ] Pushing to `main` results in a deployed change, with no manual steps
- [ ] You can identify which commit is running in production
- [ ] Two servers serve the same app behind one hostname
- [ ] Taking one server down does not take the site down
- [ ] You can articulate where session state and uploads live, and why

## Notes
