# The Craft of Technical Writing

Good technical writing has a voice — direct, structural, opinionated. The best of it does more than record: it builds an argument, teaches a system, or changes how you think about a problem. These aren't just notes; they're thinking tools. The question is whether to develop this deliberately rather than accidentally.

## Why It Matters

Systems depend on communication. Interfaces are contracts expressed in language — schema docs, API descriptions, runbooks, architecture decision records. If the writing is unclear, the interface leaks. People cross a boundary because they didn't understand where it was.

Persuasion depends on writing. Moving from a bespoke solution to a product means convincing people that the standardised version serves them better than the custom one. That's a writing problem. A technical argument can make the internal case brilliantly and still be written only for yourself. Translating that clarity for stakeholders who don't share your context is a different skill.

Teaching scales better than doing. One person can support two or three complex projects. One good document can support dozens of people making decisions independently. The enabling team concept from Team Topologies works through documentation and training, not through being present in every conversation.

## Development Areas

### Architecture Decision Records (ADRs)

Lightweight documents that capture: the decision, the context, the options considered, the trade-offs, and what would change your mind. A body of technical writing tends to accumulate conclusions (PostgreSQL-first, DV2 over Kimball, Rust for serving) but not the deliberation behind them. ADRs make reasoning searchable and revisitable.

Format: Title / Status / Context / Decision / Consequences. One page max. Store alongside code or in a dedicated decisions folder. Michael Nygard's original blog post defines the format. The "what would change my mind" section is the most valuable part — it turns a static decision into a living hypothesis.

### Writing for Different Audiences

Technical notes usually write well for one audience: a technically deep peer. Three other audiences matter:

**Executives** — care about outcomes, timelines, risks. Not how the pipeline works, but what problem it solves, how long until it's reliable, and what happens if it fails. Practice: take any technical note and rewrite it as 3 bullet points for someone who won't read past the first paragraph.

**New team members** — need context, not conclusions. They need to know *why* PostgreSQL-first, not just *that* PostgreSQL-first. The reasoning matters more than the decision because they'll encounter situations where the reasoning applies but the specific decision doesn't. Good onboarding docs are a strong technical argument rewritten with empathy for someone who wasn't there.

**Customers** — need confidence, not complexity. They care that their data is safe, their system works, and someone competent is watching. Technical depth erodes confidence if it's not framed as mastery. Practice: explain your architecture without using any technical jargon.

### Structured Thinking Through Writing

There's a style of technical writing that isn't documentation — it's *thinking*. Essays like these are already this. Developing it means:

**Claim-evidence-implication structure.** State the claim ("bespoke projects don't scale"). Provide evidence ("one person can manage 2-3 complex projects; we have 10+"). Draw the implication ("we must either reduce scope or change structure"). Most technical arguments fail because they skip from claim to implication without evidence, or present evidence without a clear claim.

**Steel-manning.** Before arguing for PostgreSQL-first, write the strongest possible case for Snowflake-first. Before arguing for a standardised product, write the strongest case for deep bespoke service. This isn't balance for its own sake — it's testing whether your conviction survives contact with the best counterargument. A good benchmark does this naturally (PostgreSQL 100-1000x for OLTP, Snowflake 10-100x for OLAP). More of this.

**Compression.** The best technical notes are the short ones with high density. A single dense line — "any intelligent fool can make things bigger" — can compress an entire argument for simplicity. Practice taking a 500-line note and compressing it to 50 lines without losing the core insight. If you can't, the core insight isn't clear yet.

## Resources and Practice

**Books:**
- *On Writing Well* — William Zinsser (the classic on clear nonfiction prose)
- *The Sense of Style* — Steven Pinker (why academic/technical writing goes wrong and how to fix it)
- *Several Short Sentences About Writing* — Verlyn Klinkenborg (radical brevity; every sentence earns its place)
- *Docs for Developers* — Bhatti et al. (technical documentation specifically)

**Practice habits:**
- Write one ADR per week for decisions already made (backfill the reasoning)
- Rewrite one technical note for a non-technical audience each month
- Keep a "compression journal" — take long notes and distill them to their essential claim
- Read your own writing aloud; cut every word that doesn't earn its place

A working body of technical notes is already a body of work. The development area isn't learning to write — it's learning to write *for different purposes and audiences* with the same clarity you already bring to technical thinking.
