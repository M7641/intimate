---
title: Cocoon Docs
description: Reference documentation and decision log for the cocoon application family.
---

This site documents **cocoon** — the Rust workspace holding the warehouse-facing
application family (`tako`, `data_view`, `redhouse`, `snowhouse`, `nyx`, `augur`) and the
crates they share.

It exists for two distinct reasons, kept deliberately apart in the sidebar:

## Applications

What each application **is** and how to run it — the reference half. Each page is
the human-facing companion to that app's source `README.md` and `Cargo.toml`,
not a copy of them.

## Decisions

Why each choice was made, recorded **over time**. This is the half that source
code and commit messages lose: the alternatives we weighed, the constraint that
tipped the balance, and what we deliberately gave up. New decisions are appended
as numbered records — see [the decision log](/decisions/overview/).

:::note
The repo-wide map lives in the root `ARCHITECTURE.md`; the build/tooling story
lives in the root `README.md`. This site is scoped to cocoon and to the
reasoning behind it.
:::
