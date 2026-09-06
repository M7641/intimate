"""SQLite storage — episodes (raw memory) + facts (bi-temporal semantic memory)
+ FTS5 index. The *stand-in* for a pgvector / Qdrant / LanceDB.

The conceptual core is the **bi-temporality** of the `facts` table:

    valid_from  : the tick at which the fact became true (validity time)
    valid_to    : the tick at which it stopped being true — NULL = still true today
    invalidated_by : the id of the fact that replaced it (revision chain)

We never overwrite or delete a fact. When the user says "I moved", we do not
`UPDATE location = 'Lisbon'`: we *close* the old fact (`valid_to = current tick`)
and insert a new one. Two properties fall out for free:

  * **default retrieval** = only current facts (`valid_to IS NULL`)
    -> the agent never re-injects stale information;
  * **time-travel** = "what did the agent know at tick T?" is a query, not log
    archaeology. Essential for audit ("why did the agent answer that?") and
    reproducibility.

"Time" is a **logical counter** (`tick`), not the wall clock: deterministic, so
the demo and tests are reproducible byte for byte.
"""

from __future__ import annotations

import sqlite3
from array import array
from dataclasses import dataclass
from pathlib import Path

from saga import embed

DEFAULT_DB = Path(__file__).resolve().parent.parent / ".saga" / "memory.db"


@dataclass
class FactRow:
    id: int
    subject: str
    predicate: str
    object: str
    text: str
    confidence: float
    valid_from: int
    valid_to: int | None
    session: str

    @property
    def key(self) -> str:
        return f"{self.subject}.{self.predicate}"


_SCHEMA = """
CREATE TABLE IF NOT EXISTS meta (k TEXT PRIMARY KEY, v INTEGER);

CREATE TABLE IF NOT EXISTS episodes (
    id        INTEGER PRIMARY KEY,
    tick      INTEGER NOT NULL,
    session   TEXT NOT NULL,
    role      TEXT NOT NULL,
    content   TEXT NOT NULL,
    embedding BLOB NOT NULL
);

CREATE TABLE IF NOT EXISTS facts (
    id             INTEGER PRIMARY KEY,
    subject        TEXT NOT NULL,
    predicate      TEXT NOT NULL,
    object         TEXT NOT NULL,
    text           TEXT NOT NULL,
    confidence     REAL NOT NULL,
    embedding      BLOB NOT NULL,
    session        TEXT NOT NULL,
    source_episode INTEGER,
    valid_from     INTEGER NOT NULL,
    valid_to       INTEGER,
    invalidated_by INTEGER
);
CREATE INDEX IF NOT EXISTS facts_key ON facts(subject, predicate);
CREATE INDEX IF NOT EXISTS facts_open ON facts(valid_to);

CREATE VIRTUAL TABLE IF NOT EXISTS facts_fts USING fts5(
    text, content='facts', content_rowid='id'
);
CREATE TRIGGER IF NOT EXISTS facts_ai AFTER INSERT ON facts BEGIN
    INSERT INTO facts_fts(rowid, text) VALUES (new.id, new.text);
END;
"""


class Store:
    """Connection + schema + low-level operations. No memory logic here (that
    lives in `memory.py`) — persistence only."""

    def __init__(self, path: Path = DEFAULT_DB) -> None:
        self.path = path
        path.parent.mkdir(parents=True, exist_ok=True)
        self.db = sqlite3.connect(path)
        self.db.row_factory = sqlite3.Row
        self.db.executescript(_SCHEMA)
        self.db.execute("INSERT OR IGNORE INTO meta(k, v) VALUES ('tick', 0)")
        self.db.commit()

    # ----- logical clock ---------------------------------------------------
    def tick(self) -> int:
        """Read the current tick without incrementing it."""
        return self.db.execute("SELECT v FROM meta WHERE k='tick'").fetchone()[0]

    def advance(self) -> int:
        """Increment then return the new tick. One write = one tick."""
        self.db.execute("UPDATE meta SET v = v + 1 WHERE k='tick'")
        return self.tick()

    # ----- episodes (raw memory) ------------------------------------------
    def add_episode(self, session: str, role: str, content: str, tick: int) -> int:
        cur = self.db.execute(
            "INSERT INTO episodes(tick, session, role, content, embedding) VALUES (?,?,?,?,?)",
            (tick, session, role, content, embed.to_blob(embed.embed(content))),
        )
        return int(cur.lastrowid)

    def recent_episodes(self, session: str, limit: int = 6) -> list[sqlite3.Row]:
        return list(
            self.db.execute(
                "SELECT * FROM episodes WHERE session=? ORDER BY tick DESC LIMIT ?",
                (session, limit),
            )
        )

    # ----- facts (bi-temporal semantic memory) ----------------------------
    def current_fact(self, subject: str, predicate: str) -> sqlite3.Row | None:
        """The current fact for a single-valued key (e.g. location)."""
        return self.db.execute(
            "SELECT * FROM facts WHERE subject=? AND predicate=? AND valid_to IS NULL",
            (subject, predicate),
        ).fetchone()

    def current_fact_obj(
        self, subject: str, predicate: str, obj: str
    ) -> sqlite3.Row | None:
        """The current fact for a multi-valued key (object is part of the key)
        — used for idempotency: "I prefer Rust" twice = one fact."""
        return self.db.execute(
            "SELECT * FROM facts WHERE subject=? AND predicate=? AND object=? "
            "COLLATE NOCASE AND valid_to IS NULL",
            (subject, predicate, obj),
        ).fetchone()

    def close_fact(self, fact_id: int, at_tick: int, replaced_by: int) -> None:
        """Close a fact (make it stale) instead of deleting it."""
        self.db.execute(
            "UPDATE facts SET valid_to=?, invalidated_by=? WHERE id=?",
            (at_tick, replaced_by, fact_id),
        )

    def insert_fact(
        self,
        *,
        subject: str,
        predicate: str,
        obj: str,
        text: str,
        confidence: float,
        session: str,
        source_episode: int,
        valid_from: int,
    ) -> int:
        cur = self.db.execute(
            """INSERT INTO facts
               (subject, predicate, object, text, confidence, embedding,
                session, source_episode, valid_from, valid_to, invalidated_by)
               VALUES (?,?,?,?,?,?,?,?,?,NULL,NULL)""",
            (
                subject,
                predicate,
                obj,
                text,
                confidence,
                embed.to_blob(embed.embed(text)),
                session,
                source_episode,
                valid_from,
            ),
        )
        return int(cur.lastrowid)

    def facts_asof(self, tick: int | None = None) -> list[tuple[FactRow, array]]:
        """All facts valid *at the given tick* (None = now), with their vector.
        This is the primitive for both retrieval AND time-travel."""
        if tick is None:
            rows = self.db.execute("SELECT * FROM facts WHERE valid_to IS NULL")
        else:
            rows = self.db.execute(
                "SELECT * FROM facts WHERE valid_from <= ? AND (valid_to IS NULL OR valid_to > ?)",
                (tick, tick),
            )
        out: list[tuple[FactRow, array]] = []
        for r in rows:
            out.append((_row_to_fact(r), embed.from_blob(r["embedding"])))
        return out

    def fts_match(self, query: str, asof: int | None = None) -> dict[int, int]:
        """Return {fact_id: rank} for valid facts matching the FTS query. The FTS
        query is sanitised (FTS5 operators are stripped)."""
        cleaned = " OR ".join(t for t in _fts_terms(query)) or '""'
        try:
            rows = self.db.execute(
                "SELECT rowid FROM facts_fts WHERE facts_fts MATCH ? ORDER BY rank",
                (cleaned,),
            ).fetchall()
        except sqlite3.OperationalError:
            return {}
        valid = {f.id for f, _ in self.facts_asof(asof)}
        return {
            r["rowid"]: i for i, r in enumerate(r for r in rows if r["rowid"] in valid)
        }

    def all_facts(self) -> list[FactRow]:
        return [
            _row_to_fact(r) for r in self.db.execute("SELECT * FROM facts ORDER BY id")
        ]

    def commit(self) -> None:
        self.db.commit()

    def close(self) -> None:
        self.db.close()


def _row_to_fact(r: sqlite3.Row) -> FactRow:
    return FactRow(
        id=r["id"],
        subject=r["subject"],
        predicate=r["predicate"],
        object=r["object"],
        text=r["text"],
        confidence=r["confidence"],
        valid_from=r["valid_from"],
        valid_to=r["valid_to"],
        session=r["session"],
    )


import re  # noqa: E402

_WORD = re.compile(r"[A-Za-z0-9]+")


def _fts_terms(query: str) -> list[str]:
    """Alphanumeric tokens only -> no FTS5 syntax injection, ever."""
    return [w for w in _WORD.findall(query.lower()) if len(w) > 1]
