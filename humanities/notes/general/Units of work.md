# Tracking ad-hoc client work: methods and when to use them

The fundamental tension with ad-hoc client work is that itemisation overhead can exceed the work itself, while clients resist feeling nickel-and-dimed — yet without itemisation, trust erodes and scope creeps. Different industries have resolved this in interesting ways.

## How other industries handle it

**Law firms** represent the extreme of granularity: 6-minute billing increments logged against "matters," where every call, email, and document review is captured. Software like Clio or Elite 3E makes the overhead tolerable, but clients tolerate it only because it's the industry norm. Accountants work similarly, though they lean more heavily on retainers with scope defined in engagement letters and out-of-scope work tracked separately.

**Creative agencies** generally reject line-item hourly billing. They work in retainers paired with "sprints" or "cycles" — a fixed monthly capacity of hours or tickets, with anything above that requiring a separate estimate and sign-off.

**Management consultancies** blend approaches: project-based with phase gates, formal change requests for scope shifts, and internal time tracking for utilisation even when the client sees a fixed price.

**Architects and civil engineers** use phased fixed fees with formal change orders — the change order is the cultural mechanism that keeps the relationship clean.

The common threads across all of them: an explicit scope document, a formal or informal change request process, and — for anything ongoing and ad-hoc — a retainer-plus-overflow model rather than pure hourly.

## Comparison of billing methods

| Method                                       | How it works                                                                                                   | Best for                                                                                                                              | Watch out for                                                                                                                              |
| -------------------------------------------- | -------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------ |
| **Pure time & materials (hourly)**           | Log hours, invoice monthly at a fixed rate                                                                     | High-trust relationships; genuinely unpredictable work; early phases where scope is unclear                                           | Clients hate uncertainty; requires disciplined weekly reporting; incentivises slowness unless trust is strong                              |
| **Retainer (block of hours)**                | Client commits to e.g. 20/40/80 hours/month at rate X; overflow billed hourly                                  | Ongoing ad-hoc work with roughly predictable volume; clients who want budget certainty; relationships you want to stabilise           | Unused hours — decide upfront if they roll over (non-rolling is simpler and more profitable); under-utilisation breeds guilt on both sides |
| **Retainer + tickets (hybrid)**              | Retainer hours, but every request becomes a ticket with estimate + actuals; monthly report shows what was done | **The default recommendation for ongoing software work.** Maintenance, small features, bug fixes, minor tweaks for an existing client | Requires real ticket discipline — Slack/email requests must be converted to tickets or the system collapses                                |
| **Per-ticket fixed pricing (S/M/L)**         | Each ticket priced at a tier (e.g. £150 / £500 / £1,500) regardless of actual hours                            | High-volume, similar-shaped work; clients who want radical predictability per item; productised services                              | Mis-sizing is expensive; only averages out at volume; tempts you to cut corners on tickets that ran long                                   |
| **Subscription / "unlimited small changes"** | Flat monthly fee covers all "small" requests                                                                   | Marketing sites, WordPress support, clients with a steady drip of tiny jobs — _if_ "small" is surgically defined                      | Almost always collapses into unbounded scope; "small" becomes "whatever the client wants"; high burnout risk                               |
| **Fixed-price project + change orders**      | Scoped SOW with defined deliverables; anything outside scope goes through a formal change request              | Discrete projects with clear boundaries (migrations, rebuilds, integrations); clients who need capex-style budgeting                  | Doesn't fit ad-hoc work at all; change-order friction sours relationships if overused; requires strong upfront scoping skills              |
| **Value-based pricing**                      | Fee tied to business outcome (e.g. % of revenue uplift, flat fee for "launch this product")                    | Senior consultants with measurable impact; strategic work where hours are a bad proxy for value                                       | Hard to apply to maintenance/tweaks; requires you to be able to attribute outcomes to your work                                            |

## Recommended setup for software engineering

The recipe that good agencies and senior freelancers converge on:

**Every request becomes a ticket** — even "change this button colour." This is the non-negotiable foundation. Tickets are simultaneously the unit of work, the unit of communication, and the unit of billing. Use Linear, GitHub Issues, or Jira depending on what the client already lives in. Each ticket gets an estimate before work starts (5 min, 30 min, 2h, 1 day) and actual time logged after. A shared board the client can see eliminates most "what's happening with X?" conversations and makes prioritisation the client's responsibility rather than yours.

**Pair it with a retainer.** The retainer-plus-tickets hybrid is the sweet spot for ongoing ad-hoc work. It gives the client budget predictability, gives you revenue predictability, and the ticket discipline keeps everything accountable.

### Monthly rhythm

1. Requests come in
2. Become tickets
3. Get estimated
4. Get prioritised by the client
5. Get done
6. Hours logged
7. Monthly report showing tickets closed, estimates vs. actuals, retainer hours consumed, and any overflow billed

### Practical toolstack

- **Linear** for tickets — clean, fast, clients find it intuitive
- **Harvest or Toggl** for time tracking with Linear integration so a timer starts from the ticket
- **Monthly PDF** exported from Harvest as the invoice backup

Some people run everything through **Jira + Tempo**, which is heavier but fine if the client is already a Jira shop.

## Failure modes to watch for

- **Not logging time on "quick" tweaks** — they add up to 20-30% of real effort over a month
- **Skipping estimates** — clients then assume everything is trivial
- **Batching a month of work into one opaque invoice line** — breeds distrust fast
- **Letting Slack or email become a parallel request channel** that bypasses the ticket system — force everything through tickets, even if you have to create the ticket yourself from a Slack message
- **Treating communication as free** — a 20-minute clarification call is part of delivering the work. Make it billable and bake it into the ticket's actuals.
