"""End-to-end narrative demo. `saga demo` or `python -m saga.demo`.

Tells the story of an agent that remembers a user across TWO separate sessions,
resolves a contradiction (a move) through temporal invalidation, then proves you
can query the past (time-travel).

All the mechanics live in the other modules; this is presentation only.
"""

from __future__ import annotations

import sys
import tempfile
from pathlib import Path

from saga.memory import Memory


def _section(title: str) -> None:
    print(f"\n\033[1m── {title} ──\033[0m")


def _say(session: str, role: str, content: str, mem: Memory) -> None:
    colour = "\033[36m" if role == "user" else "\033[35m"
    print(f"  [{session}] {colour}{role}\033[0m: {content}")
    w = mem.remember(session, role, content)
    for f in w.added:
        print(f"        \033[32m+ fact\033[0m  {f}")
    for f in w.invalidated:
        print(f"        \033[31m~ closed\033[0m  {f}")


def main(db: Path | None = None) -> int:
    db = db or Path(tempfile.mkdtemp()) / "demo.db"
    mem = Memory(db)

    _section("1. Session 'monday' — the agent meets the user")
    print("  The agent is stateless. Every message is ingested: the raw episode")
    print("  is stored, and an extraction call (regex here, an LLM in prod)")
    print("  consolidates the durable facts.")
    _say(
        "monday",
        "user",
        "Hi, I'm Alex. I'm a data engineer and I'm based in Manchester.",
        mem,
    )
    _say(
        "monday",
        "user",
        "I prefer Rust for systems work and I love strong typing.",
        mem,
    )
    _say("monday", "agent", "Nice to meet you Alex — noted.", mem)
    monday_tick = mem.store.tick()

    _section("2. Session 'friday' — NEW session, zero chat history")
    print("  This is THE property that distinguishes a context database from a")
    print("  conversation history: semantic memory crosses sessions. The agent")
    print("  has no messages on friday, and yet:")
    ctx = mem.assemble_context("friday", "what do you know about me?")
    print("\n  block injected into the system prompt before the LLM call:")
    for line in ctx.splitlines():
        print(f"    \033[2m{line}\033[0m")

    _section("3. Contradiction — the user moves")
    print("  We do not overwrite the old fact: we CLOSE it and insert the new one.")
    _say("friday", "user", "Actually I moved to Lisbon last month.", mem)

    _section("4. Hybrid retrieval — freshness is guaranteed by storage")
    print("  A query about location can no longer surface 'Manchester':")
    for s in mem.recall("the user location", k=3):
        print(
            f"    \033[32m{s.score:.4f}\033[0m  {s.fact.text}   \033[2m[{s.why}]\033[0m"
        )

    _section("5. Time-travel — 'what did the agent know on monday?'")
    print(f"  Replays retrieval as it would have been at tick {monday_tick}:")
    for s in mem.recall("the user location", k=1, asof=monday_tick):
        print(f"    monday  →  {s.fact.text}")
    for s in mem.recall("the user location", k=1):
        print(f"    now     →  {s.fact.text}")

    _section("6. What the agent re-injects on friday, after the move")
    ctx = mem.assemble_context("friday", "remind me where I'm based and what I like")
    for line in ctx.splitlines():
        print(f"    \033[2m{line}\033[0m")

    print("\n\033[1mSummary\033[0m: episodic + semantic, cross-session, contradiction")
    print("resolved by bi-temporality, hybrid retrieval, time-travel — the core")
    print("of a context database for agents, offline, with no model.\n")
    mem.close()
    return 0


if __name__ == "__main__":
    sys.exit(main())
