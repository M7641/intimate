"""SQL safety validation for LLM-generated queries.

Three guarantees this module makes about any SQL it returns:

1. **Read-only**: the root statement is SELECT (or WITH ... SELECT).
   Any DML/DDL keyword anywhere in the tree is rejected.
2. **Single-statement**: stripped of trailing whitespace and the final
   semicolon, no further semicolon may appear. Protects against
   stacked queries.
3. **Allowlisted tables**: every referenced table appears in the
   curated schema. The LLM cannot probe tables outside the pilot's
   declared surface.

A LIMIT is injected if absent, capped at ``DEFAULT_LIMIT``.

We use sqlglot (Redshift dialect) for parsing and AST walking — never
regex. The whole point of sqlglot is to defeat the cleverness of
"smuggle a DELETE inside a comment" or "use a CTE to disguise an
INSERT": the AST is what executes, not the text.
"""

from dataclasses import dataclass

import sqlglot
import sqlglot.expressions as exp

from common_py.ask_warehouse.schema import CuratedSchema, load_curated_schema

DEFAULT_LIMIT = 10000


@dataclass(frozen=True, slots=True)
class ParsedSQL:
    """The model emitted a ```sql ... ``` fence — ``sql`` is its contents."""

    sql: str


@dataclass(frozen=True, slots=True)
class ParsedUnsure:
    """The model began its output with ``-- UNSURE``: a data question it
    couldn't map to a table. ``reason`` is the one-line explanation."""

    reason: str


@dataclass(frozen=True, slots=True)
class ParsedAnswer:
    """The model emitted plain prose — a non-data answer (greeting, meta
    question, clarification, etc.) to be surfaced verbatim."""

    text: str


LLMOutput = ParsedSQL | ParsedUnsure | ParsedAnswer

# These expression classes are unconditionally rejected anywhere in the AST.
# sqlglot represents each DDL / DML verb as its own class — we lock them
# all out, no exceptions.
_FORBIDDEN_EXPRS: tuple[type[exp.Expression], ...] = (
    exp.Insert,
    exp.Update,
    exp.Delete,
    exp.Drop,
    exp.Create,
    exp.Alter,
    exp.TruncateTable,
    exp.Grant,
    exp.Command,  # catches things like CALL, COPY, UNLOAD that sqlglot lumps here
)


class UnsafeSqlError(ValueError):
    """Raised when LLM-generated SQL fails any safety check."""


def _strip_one_trailing_semicolon(sql: str) -> str:
    stripped = sql.strip()
    if stripped.endswith(";"):
        stripped = stripped[:-1].rstrip()
    return stripped


def _has_extra_semicolon(sql: str) -> bool:
    """Detect statement chaining after we've removed at most one trailing ``;``."""
    return ";" in _strip_one_trailing_semicolon(sql)


def _collect_table_names(tree: exp.Expression) -> set[str]:
    """Walk the AST and return the lower-cased names of every table reference."""
    names: set[str] = set()
    for table in tree.find_all(exp.Table):
        # sqlglot.Table carries db / catalog / name parts. Match against any
        # plausible spelling the LLM might emit (short name, schema.table,
        # catalog.schema.table) — the allowlist already includes all three.
        name = table.name.lower() if table.name else ""
        if not name:
            continue
        names.add(name)
        if table.db:
            names.add(f"{table.db}.{name}".lower())
        if table.catalog:
            names.add(f"{table.catalog}.{table.db}.{name}".lower())
    return names


def _collect_cte_names(tree: exp.Expression) -> set[str]:
    """CTE aliases (the ``x`` in ``WITH x AS (SELECT …)``) are defined locally —
    references to them must be excluded from the table allowlist check."""
    return {
        cte.alias_or_name.lower() for cte in tree.find_all(exp.CTE) if cte.alias_or_name
    }


def _ensure_limit(tree: exp.Expression, limit: int) -> exp.Expression:
    """Inject a LIMIT clause on the outermost SELECT if absent."""
    if not isinstance(tree, exp.Select):
        # WITH ... SELECT wraps the SELECT in a Subquery / Query — sqlglot's
        # ``select`` method walks down to the underlying SELECT for us.
        select = tree.find(exp.Select)
        if select is None:
            return tree
        if select.args.get("limit") is None:
            select.limit(limit, copy=False)
        return tree
    if tree.args.get("limit") is None:
        tree = tree.limit(limit, copy=False)
    return tree


def validate_and_prepare(
    sql: str,
    *,
    schema: CuratedSchema | None = None,
    limit: int = DEFAULT_LIMIT,
) -> str:
    """Validate ``sql``, inject LIMIT if absent, and return the rewritten SQL.

    Raises :class:`UnsafeSqlError` if any safety check fails. The error
    message is safe to surface to the caller — it never contains user
    data, only the LLM's drafted SQL fragment.
    """
    if _has_extra_semicolon(sql):
        raise UnsafeSqlError("multi-statement payload rejected (extra ';' found)")

    cleaned = _strip_one_trailing_semicolon(sql)

    try:
        tree = sqlglot.parse_one(cleaned, dialect="redshift")
    except sqlglot.errors.ParseError as e:
        raise UnsafeSqlError(f"unparseable SQL: {e}") from e

    if tree is None:
        raise UnsafeSqlError("empty SQL")

    # The root must be SELECT or a WITH that wraps a SELECT.
    root = tree
    if isinstance(root, exp.With):
        root = root.this
    if not isinstance(root, exp.Select):
        raise UnsafeSqlError(
            f"root statement must be SELECT, got {type(root).__name__}"
        )

    # No forbidden verbs anywhere in the tree (including inside CTEs).
    for node in tree.walk():
        node_expr = node[0] if isinstance(node, tuple) else node
        if isinstance(node_expr, _FORBIDDEN_EXPRS):
            raise UnsafeSqlError(
                f"forbidden statement type: {type(node_expr).__name__}"
            )

    # Allowlist tables. Exclude CTE aliases, which are defined inline by
    # the query itself (``WITH x AS …`` makes ``x`` a valid name for the
    # scope of the statement).
    curated = schema if schema is not None else load_curated_schema()
    allowed = curated.allowed_table_names() | _collect_cte_names(tree)
    referenced = _collect_table_names(tree)
    extra = referenced - allowed
    # A table-less SELECT (e.g. ``SELECT 1``) is fine.
    if extra:
        raise UnsafeSqlError(
            f"references table(s) outside the curated allowlist: {sorted(extra)}"
        )

    # Inject LIMIT if absent.
    tree = _ensure_limit(tree, limit)

    return tree.sql(dialect="redshift")


def parse_llm_output(raw: str) -> LLMOutput:
    """Classify the model output as SQL, UNSURE, or plain-text answer.

    Three response modes are supported, in priority order:

    * If the first line starts with ``-- UNSURE`` → :class:`ParsedUnsure`.
    * If the text contains a ```` ```sql ... ``` ```` fence → :class:`ParsedSQL`
      with the fence contents.
    * Otherwise → :class:`ParsedAnswer` carrying the full text verbatim
      (the model chose to answer conversationally instead of querying).

    Raises:
        :class:`UnsafeSqlError` only when a ``sql`` fence is *opened*
        but never closed — a malformed output we can't safely interpret.
        Empty / pure-whitespace input is treated as a (vacuous) answer.
    """
    text = raw.strip()
    if not text:
        return ParsedAnswer(text="")

    first_line = text.splitlines()[0].strip()
    if first_line.startswith("-- UNSURE"):
        reason = (
            first_line[len("-- UNSURE") :].strip(": ").strip() or "(no reason given)"
        )
        return ParsedUnsure(reason=reason)

    fence_open = "```sql"
    lower = text.lower()
    start = lower.find(fence_open)
    if start == -1:
        # No ```sql fence → treat the whole text as a conversational
        # answer. We deliberately do *not* fall back to a bare ``` fence
        # here: a model might quote itself or use code formatting for
        # emphasis, and parsing that as SQL would surprise the user.
        return ParsedAnswer(text=text)

    body_start = start + len(fence_open)
    end = text.find("```", body_start)
    if end == -1:
        raise UnsafeSqlError("unterminated SQL code fence in model output")

    return ParsedSQL(sql=text[body_start:end].strip())
