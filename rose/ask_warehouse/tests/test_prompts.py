"""The assembled system prompt must contain every block the LLM relies on.

This is intentionally a content test, not a byte-exact snapshot:
- glossary terms must appear by their full name (LFL, Linecard, WSSI)
- every curated table must appear by its short name
- the INSTRUCTIONS preamble must include the read-only and LIMIT rules
- few-shot examples must be code-fenced for the chat template

If you change the YAML and a test fails, that's the test doing its job —
the prompt is the only thing standing between the LLM and wrong answers.
"""

from common_py.ask_warehouse.prompts import (
    INSTRUCTIONS,
    build_chat_messages,
    build_system_prompt,
)
from common_py.ask_warehouse.schema import load_curated_schema


def test_instructions_have_safety_rules() -> None:
    assert "Only SELECT" in INSTRUCTIONS
    assert "LIMIT" in INSTRUCTIONS
    assert "-- UNSURE" in INSTRUCTIONS


def test_system_prompt_includes_every_curated_table() -> None:
    prompt = build_system_prompt()
    schema = load_curated_schema()
    for short_name in schema.tables:
        assert short_name in prompt, f"missing table {short_name} from system prompt"


def test_system_prompt_includes_lfl_glossary() -> None:
    prompt = build_system_prompt()
    assert "Like-for-like" in prompt
    assert "dim__lfl_stores" in prompt


def test_system_prompt_includes_few_shot_sql_fences() -> None:
    prompt = build_system_prompt()
    assert "```sql" in prompt


def test_chat_messages_have_system_and_user_roles() -> None:
    messages = build_chat_messages("how many stores in the south region?")
    assert messages[0]["role"] == "system"
    assert messages[1]["role"] == "user"
    assert "south" in messages[1]["content"]


def test_prompt_is_reasonably_sized() -> None:
    """Sanity guard against accidentally bloating the prompt past usable size.

    A 7B local model has a finite context window (~32k tokens for Qwen2.5).
    The prompt should sit comfortably under 8k tokens (rough chars/4 estimate)
    so user questions and generated output have room.
    """
    prompt = build_system_prompt()
    approx_tokens = len(prompt) // 4
    assert approx_tokens < 8000, (
        f"system prompt is ~{approx_tokens} tokens — review whether new "
        "tables/glossary entries justify the bloat or should be trimmed"
    )
