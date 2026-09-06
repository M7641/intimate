"""System-prompt assembly for the ask_warehouse LLM call.

Three static blocks (schema + glossary + few-shot examples) plus a
``INSTRUCTIONS`` preamble. The model's KV cache amortises re-reading
these on subsequent requests, so this prompt is large by design —
the marginal cost is paid once per cold start.
"""

import yaml

from common_py.ask_warehouse.mod import (
    CURATED_TABLES_PATH,
    FEW_SHOT_PATH,
    GLOSSARY_PATH,
)
from common_py.ask_warehouse.schema import CuratedSchema, load_curated_schema

INSTRUCTIONS = """\
You are a Redshift SQL assistant for Northwind Retail's merchandising warehouse.

You handle two kinds of input — pick the matching response mode:

A) **Data questions** — anything whose answer lives in the warehouse
   tables listed below. Respond with exactly one SQL statement,
   wrapped in a ```sql ... ``` code fence:

   - Only SELECT (or WITH ... SELECT). No INSERT, UPDATE, DELETE, DROP, COPY, UNLOAD, GRANT, ALTER, CREATE, TRUNCATE, CALL.
   - Always include a LIMIT clause. Default LIMIT 1000 unless the question implies otherwise.
   - Always alias aggregated columns (use AS).
   - Only reference tables that appear in the ## Tables block below.
   - Quote string literals with single quotes. Quote identifiers only if they contain non-ASCII or are reserved words.
   - Use Monday-start ISO weeks for week math (column ``date_week``).
   - Express revenue in GBP (the warehouse stores it that way already).
   - If the user implies a year-on-year comparison, default to LFL stores unless they explicitly say "all stores" or "total".

   If it *is* a data question but you do not know which table holds
   the answer, respond with a single line: ``-- UNSURE: <one-line
   reason>`` — no SQL fence, no further text.

B) **Conversational input** — greetings, small talk, meta questions
   ("what can you do?", "what tables are available?", "how do you
   work?"), clarifications, or anything that doesn't need a warehouse
   query. Reply briefly in plain prose. No SQL fence, no ``-- UNSURE``
   prefix. Keep it short (one short paragraph) and feel free to
   describe the kinds of questions you handle or reference the tables
   by name from the list below.

Glossary entries below define the retail-specific vocabulary. Use these
terms exactly as the user does and apply the suggested SQL hints.
"""


def render_schema(schema: CuratedSchema) -> str:
    """Render the curated schema YAML into prompt-friendly Markdown."""
    parts: list[str] = []
    for short_name, table in schema.tables.items():
        parts.append(f"### `{short_name}` (`{table.full_name}`)")
        parts.append(f"- **Grain**: {table.grain}")
        if table.primary_key:
            parts.append(f"- **Primary key**: ({', '.join(table.primary_key)})")
        parts.append("- **Columns**:")
        for col_name, col in table.columns.items():
            parts.append(f"  - `{col_name}` ({col.type}) — {col.desc}")
        if table.foreign_keys:
            parts.append("- **Foreign keys**:")
            for fk in table.foreign_keys:
                parts.append(f"  - `{fk.col}` → `{fk.ref}`")
        if table.typical_filters:
            parts.append("- **Typical filters**:")
            for f in table.typical_filters:
                parts.append(f"  - `{f}`")
        parts.append("")
    return "\n".join(parts)


def render_glossary(glossary_yaml: dict) -> str:
    parts: list[str] = []
    for term_key, term in glossary_yaml.get("terms", {}).items():
        parts.append(f"### {term['full']} ({term_key})")
        parts.append(term["means"].strip())
        if "sql_hint" in term:
            parts.append(f"- **SQL hint**: `{term['sql_hint']}`")
        if "default_assumption" in term:
            parts.append(f"- **Default assumption**: {term['default_assumption']}")
        if "primary_table" in term:
            parts.append(f"- **Primary table**: `{term['primary_table']}`")
        if "grain" in term:
            parts.append(f"- **Grain**: `{term['grain']}`")
        if "related_tables" in term:
            parts.append(
                f"- **Related tables**: {', '.join(f'`{t}`' for t in term['related_tables'])}"
            )
        parts.append("")
    return "\n".join(parts)


def render_examples(examples_yaml: dict) -> str:
    parts: list[str] = []
    for example in examples_yaml.get("examples", []):
        parts.append(f"**Question:** {example['question']}")
        parts.append("```sql")
        parts.append(example["sql"].rstrip())
        parts.append("```")
        if "why" in example:
            parts.append(f"*Why this query:* {example['why']}")
        parts.append("")
    return "\n".join(parts)


def build_system_prompt(
    *,
    schema_path: object = None,
    glossary_path: object = None,
    few_shot_path: object = None,
) -> str:
    """Assemble the full system prompt from the three YAMLs."""
    schema = load_curated_schema(schema_path or CURATED_TABLES_PATH)
    with open(glossary_path or GLOSSARY_PATH) as f:
        glossary = yaml.safe_load(f)
    with open(few_shot_path or FEW_SHOT_PATH) as f:
        examples = yaml.safe_load(f)
    return (
        INSTRUCTIONS
        + "\n\n## Tables\n\n"
        + render_schema(schema)
        + "\n\n## Glossary\n\n"
        + render_glossary(glossary)
        + "\n\n## Examples\n\n"
        + render_examples(examples)
    )


def build_chat_messages(question: str) -> list[dict[str, str]]:
    """Return the chat-template messages list for ``tokenizer.apply_chat_template``."""
    return [
        {"role": "system", "content": build_system_prompt()},
        {"role": "user", "content": question},
    ]
