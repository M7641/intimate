"""The API the agent sees. Assembles the four seams into three operations.

    remember(session, role, content)   ingest a turn + consolidate facts
    recall(query)                      retrieve relevant facts (hybrid)
    assemble_context(session, query)   build the block to paste into the prompt

The contract with the agent is deliberately tiny: the agent stays *stateless* (an
LLM call with no state), and all persistence lives here. That decoupling is what
distinguishes a "context database" from a chat history — the agent does not need
to know *how* its memory is stored, only to call `remember` after each turn and
`assemble_context` before.
"""

from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path

from saga import extract, retrieve
from saga.store import DEFAULT_DB, Store


@dataclass
class Written:
    """What a `remember` produced — handy for the demo and for logs."""

    episode_id: int
    added: list[str]  # new facts
    invalidated: list[str]  # facts made stale by this turn (contradictions resolved)


class Memory:
    def __init__(self, path: Path = DEFAULT_DB) -> None:
        self.store = Store(path)

    # ----- write ----------------------------------------------------------
    def remember(self, session: str, role: str, content: str) -> Written:
        """Ingest a message. Two memory levels in one call:
        1. the raw **episode** is always stored (working memory / audit);
        2. the extracted **facts** feed semantic memory, with contradiction
           resolution by key `(subject, predicate)`."""
        tick = self.store.advance()
        ep_id = self.store.add_episode(session, role, content, tick)

        added: list[str] = []
        invalidated: list[str] = []
        # We only extract from user messages: the agent's turns are not a source
        # of truth about the user (in prod, an LLM would also decide what to
        # extract from tool/agent turns).
        if role == "user":
            for fact in extract.extract(content):
                single = fact.predicate in extract.SINGLE_VALUED
                if single:
                    # single-valued: one current object; a new one replaces it.
                    prev = self.store.current_fact(fact.subject, fact.predicate)
                    if (
                        prev is not None
                        and prev["object"].lower() == fact.object.lower()
                    ):
                        continue  # already known, identical -> idempotent
                else:
                    # multi-valued: objects accumulate; we close nothing, we just
                    # dedup on the exact object.
                    if self.store.current_fact_obj(
                        fact.subject, fact.predicate, fact.object
                    ):
                        continue
                    prev = None
                new_id = self.store.insert_fact(
                    subject=fact.subject,
                    predicate=fact.predicate,
                    obj=fact.object,
                    text=fact.render(),
                    confidence=fact.confidence,
                    session=session,
                    source_episode=ep_id,
                    valid_from=tick,
                )
                if prev is not None:  # contradiction: close the old one
                    self.store.close_fact(prev["id"], at_tick=tick, replaced_by=new_id)
                    invalidated.append(f"{prev['text']}  (→ stale)")
                added.append(fact.render())

        self.store.commit()
        return Written(episode_id=ep_id, added=added, invalidated=invalidated)

    # ----- read -----------------------------------------------------------
    def recall(
        self, query: str, *, k: int = 5, asof: int | None = None
    ) -> list[retrieve.Scored]:
        """Facts relevant to `query`. `asof` = time-travel (past tick)."""
        return retrieve.retrieve(self.store, query, k=k, asof=asof)

    def assemble_context(self, session: str, query: str, *, k: int = 5) -> str:
        """The block ready to paste into the system prompt before the LLM call.

        Mixes the two memories, like a real agent does:
          * SEMANTIC — the relevant durable facts (cross-session);
          * EPISODIC — the latest turns of *this* session (immediate thread).
        """
        facts = self.recall(query, k=k)
        episodes = self.store.recent_episodes(session, limit=4)

        lines = ["<memory>"]
        lines.append("  semantic (durable, cross-session):")
        if facts:
            for s in facts:
                lines.append(f"    - {s.fact.text}")
        else:
            lines.append("    - (nothing relevant)")
        lines.append("  episodic (recent turns, this session):")
        for ep in reversed(episodes):
            lines.append(f"    {ep['role']}: {ep['content']}")
        lines.append("</memory>")
        return "\n".join(lines)

    def close(self) -> None:
        self.store.close()
