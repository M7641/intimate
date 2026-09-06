"""Rule-based fact extraction — the *stand-in* for the LLM call.

This is the most important seam to understand. A context database does not keep
raw messages as long-term memory: it **consolidates** them into structured facts
`(subject, predicate, object)` — that is *semantic* memory. In production this
consolidation is an LLM call ("read this message, return the facts about the
user as JSON"). Here we mimic it with regex patterns.

Why rules are enough for the pilot: the *value* of a semantic memory comes not
from the finesse of extraction but from what we do with it afterwards —
deduplication, contradiction detection, temporal invalidation, retrieval. All of
that machinery (in `store.py` / `retrieve.py`) is indifferent to *how* the fact
was produced. So we can swap these regexes for an LLM without touching the rest.

Honest consequence: rule-based extraction is brittle. It misses phrasings we did
not anticipate, and it has no notion of cardinality (an LLM would know that "I
prefer X" accumulates but "my name is X" replaces). So we encode that cardinality
by hand via `SINGLE_VALUED`.
"""

from __future__ import annotations

import re
from dataclasses import dataclass

# SINGLE-valued predicates: a new object *replaces* the old one (contradiction).
# Everything else is multi-valued: objects accumulate (you can prefer several
# things). That is what an LLM would infer; here we declare it.
SINGLE_VALUED = {"name", "location", "employer", "role"}


@dataclass(frozen=True)
class Fact:
    """A semantic fact. For predicates in `SINGLE_VALUED`, the key
    `(subject, predicate)` triggers contradiction detection: a new fact with the
    same key but a different `object` replaces the old one."""

    subject: str
    predicate: str
    object: str
    confidence: float = 0.9

    def render(self) -> str:
        """Readable sentence — this text is what gets embedded and FTS-indexed,
        so it is what retrieval actually "sees"."""
        return f"{self.subject} {self.predicate} {self.object}"


_END = r"(?:[.,!?]|\s+and\b|$)"  # common end boundary (also stops on "and")

# (regex, predicate, confidence, proper?) — `proper` = keep only the leading run
# of capitalised words (proper nouns: "Lisbon last month" -> "Lisbon").
_RULES: list[tuple[re.Pattern[str], str, float, bool]] = [
    (
        re.compile(r"\b(?:my name is|i am|i'm|call me)\s+([A-Za-z]+)", re.I),
        "name",
        0.97,
        True,
    ),
    (
        re.compile(
            rf"\bi(?:'m| am)? (?:a|an)\s+([a-z][a-z ]+?)(?:\s+at\b|{_END})", re.I
        ),
        "role",
        0.9,
        False,
    ),
    (re.compile(r"\b(?:at|for)\s+([A-Z][A-Za-z&]+)\b"), "employer", 0.93, True),
    (
        re.compile(
            rf"\b(?:based in|live in|living in|moved to|relocated to|i'm in)\s+([A-Za-z][\w ]+?){_END}",
            re.I,
        ),
        "location",
        0.95,
        True,
    ),
    (
        re.compile(rf"\bi prefer\s+([A-Za-z][\w +#]+?)(?:\s+for\b|{_END})", re.I),
        "preference",
        0.85,
        False,
    ),
    (
        re.compile(rf"\bi (?:love|like|enjoy)\s+([A-Za-z][\w +#]+?){_END}", re.I),
        "preference",
        0.8,
        False,
    ),
    (re.compile(rf"\bi use\s+([A-Za-z][\w +#]+?){_END}", re.I), "tool", 0.82, False),
    (
        re.compile(rf"\bmy favou?rite ([\w ]+?) is\s+([A-Za-z][\w +#]+?){_END}", re.I),
        "favorite:%s",
        0.88,
        False,
    ),
]


def _clean(s: str) -> str:
    return re.sub(r"\s+", " ", s).strip(" .!,?")


def _proper(s: str) -> str:
    """Keep the leading run of capitalised words. "Lisbon last month" ->
    "Lisbon"; "New York is great" -> "New York"; "based" -> ""."""
    out: list[str] = []
    for w in s.split():
        if w[:1].isupper():
            out.append(w)
        else:
            break
    return " ".join(out)


def extract(text: str, subject: str = "user") -> list[Fact]:
    """Message -> list of facts. Empty if nothing matches (most conversation
    turns carry no durable fact — and that is normal)."""
    facts: list[Fact] = []
    seen: set[tuple[str, str]] = set()
    for pattern, predicate, conf, proper in _RULES:
        for m in pattern.finditer(text):
            if "%s" in predicate:  # dynamic predicate: "favorite X is Y"
                pred = predicate % _clean(m.group(1)).lower().replace(" ", "_")
                obj = _clean(m.group(2))
            else:
                pred = predicate
                obj = _clean(m.group(1))
            if proper:
                obj = _proper(obj)
            if not obj or len(obj) > 60:
                continue
            key = (pred, obj.lower())
            if key in seen:
                continue
            seen.add(key)
            facts.append(
                Fact(subject=subject, predicate=pred, object=obj, confidence=conf)
            )
    return facts
