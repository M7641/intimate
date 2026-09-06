"""The model-free helpers in the extractor: JSON parsing, dtype, voting.

The model itself isn't loaded here — only the pure pieces around it, plus the
extract orchestration with the tokenizer/model mocked. The chatty-output JSON
parser is the one most worth pinning.
"""

from __future__ import annotations

import pytest
import torch

import extract.extractor.extractor as em
from extract.domains import get_schema
from extract.extractor.extractor import (
    TextExtractor,
    dtype_for,
    parse_json,
    parse_json_array,
    quantization_config,
    vote,
)


def test_parse_json_extracts_a_clean_object():
    assert parse_json('{"a": 1}') == {"a": 1}


def test_parse_json_ignores_prose_and_code_fences():
    text = 'Sure!\n```json\n{"a": 1, "b": "x"}\n```\nHope that helps.'
    assert parse_json(text) == {"a": 1, "b": "x"}


def test_parse_json_returns_empty_on_non_json():
    assert parse_json("there is no json here") == {}


def test_parse_json_rejects_top_level_non_object():
    # A bare array is valid JSON but not the dict the schema expects.
    assert parse_json("[1, 2, 3]") == {}


def test_parse_json_stops_at_end_of_object_ignoring_trailing_prose():
    assert parse_json('{"a": 1} and that is my final answer }') == {"a": 1}


def test_parse_json_returns_first_object_when_several():
    assert parse_json('{"a": 1} {"b": 2}') == {"a": 1}


def test_parse_json_skips_a_false_start_brace():
    # The first "{" does not begin valid JSON; scanning finds the real object.
    assert parse_json('prefix {not json} {"a": 1}') == {"a": 1}


def test_parse_json_array_extracts_a_clean_array():
    assert parse_json_array('[{"a": 1}, {"b": 2}]') == [{"a": 1}, {"b": 2}]


def test_parse_json_array_ignores_prose_and_code_fences():
    text = 'Sure!\n```json\n[{"a": 1}]\n```\nHope that helps.'
    assert parse_json_array(text) == [{"a": 1}]


def test_parse_json_array_returns_empty_on_no_array():
    assert parse_json_array('just an object: {"a": 1}') == []


def test_parse_json_array_skips_a_false_start_bracket():
    assert parse_json_array('prefix [not json] [{"a": 1}]') == [{"a": 1}]


def test_dtype_for_cpu_is_float32():
    assert dtype_for(torch.device("cpu")) == torch.float32


# --- extract() orchestration, with the model/tokenizer mocked (no weights) ---


class FakeBatch(dict):
    def to(self, device):  # the real tokenizer output exposes .to(device)
        return self


class FakeTokenizer:
    def __init__(self):
        self.padding_side = "right"
        self.pad_token_id = 0
        self.eos_token = "<eos>"
        # Chatty output by default; tests override to drive the grouped path.
        self.reply = 'Sure: {"category": "dress", "primary_colour": "red"} ✅'

    def apply_chat_template(self, messages, tokenize, add_generation_prompt):
        self.last_messages = messages
        return "PROMPT"

    def __call__(self, text, return_tensors, padding=False):
        # One row per text → exercises both single and batched paths.
        return FakeBatch(input_ids=torch.zeros((len(text), 3), dtype=torch.long))

    def batch_decode(self, trimmed, skip_special_tokens):
        # One reply per row; the parser must still recover each object.
        return [self.reply] * trimmed.shape[0]


class FakeModel:
    def __init__(self):
        self.calls = 0  # lets tests count generations (grouped vs fallback)

    def to(self, device):
        return self

    def eval(self):
        return self

    def generate(self, input_ids=None, **kw):
        self.calls += 1
        n = input_ids.shape[0] if input_ids is not None else 1
        return torch.zeros((n, 5), dtype=torch.long)  # 3 prompt + 2 generated


def build(monkeypatch, **kwargs) -> TextExtractor:
    monkeypatch.setattr(
        em,
        "AutoTokenizer",
        type(
            "T", (), {"from_pretrained": staticmethod(lambda *a, **k: FakeTokenizer())}
        ),
    )
    monkeypatch.setattr(
        em,
        "AutoModelForCausalLM",
        type("M", (), {"from_pretrained": staticmethod(lambda *a, **k: FakeModel())}),
    )
    return TextExtractor("fake-model", device=torch.device("cpu"), **kwargs)


def test_extract_parses_and_validates(monkeypatch):
    ex = build(monkeypatch)
    result = ex.extract(get_schema("clothing"), "a red maxi dress")

    # The user message is plain-text content (no multimodal image parts).
    user_content = ex.tokenizer.last_messages[1]["content"]
    assert isinstance(user_content, str)
    # The chatty JSON was parsed and validated through the schema.
    assert result["category"] == "dress"
    assert result["primary_colour"] == "red"


def test_extract_batch_returns_one_dict_per_row(monkeypatch):
    ex = build(monkeypatch)
    out = ex.extract_batch(get_schema("clothing"), ["a red dress", "a top"])
    assert len(out) == 2
    assert all(row["category"] == "dress" for row in out)
    # Decoder-only batched generation must left-pad.
    assert ex.tokenizer.padding_side == "left"


def test_extract_consistent_votes_and_scores_confidence(monkeypatch):
    ex = build(monkeypatch)
    consensus, confidence = ex.extract_consistent(
        get_schema("clothing"), "a red dress", samples=3
    )
    assert consensus["category"] == "dress"
    # All three (identical) samples agree.
    assert confidence["category"] == 1.0


def test_constrained_without_outlines_raises_clear_error(monkeypatch):
    ex = build(monkeypatch, constrained=True)
    with pytest.raises(RuntimeError, match="outlines"):
        ex.extract(get_schema("clothing"), "a red dress")


def test_extract_grouped_parses_one_array_reply(monkeypatch):
    ex = build(monkeypatch)
    ex.tokenizer.reply = (
        '[{"category": "dress", "primary_colour": "red"}, '
        '{"category": "top", "primary_colour": "blue"}]'
    )
    out = ex.extract_grouped(get_schema("clothing"), ["a red dress", "a blue top"])
    assert [r["category"] for r in out] == ["dress", "top"]
    # The whole group rode a single generation.
    assert ex.model.calls == 1
    # The grouped user turn numbers the descriptions.
    assert "1. a red dress" in ex.tokenizer.last_messages[1]["content"]


def test_extract_grouped_falls_back_per_row_on_wrong_count(monkeypatch, caplog):
    ex = build(monkeypatch)
    ex.tokenizer.reply = '[{"category": "dress"}]'  # 1 object for 2 descriptions
    with caplog.at_level("WARNING"):
        out = ex.extract_grouped(get_schema("clothing"), ["a red dress", "a blue top"])
    assert "falling back" in caplog.text
    # 1 grouped attempt + 2 per-row recoveries; rows survive, none nulled.
    assert ex.model.calls == 3
    assert len(out) == 2
    assert all(r["category"] == "dress" for r in out)


# --- pure helpers ---


def test_vote_picks_majority_and_reports_agreement():
    schema = get_schema("clothing")
    runs = [
        {"category": "dress", "primary_colour": "red"},
        {"category": "dress", "primary_colour": "red"},
        {"category": "top", "primary_colour": None},
    ]
    consensus, confidence = vote(runs, schema)
    assert consensus["category"] == "dress"
    assert confidence["category"] == pytest.approx(2 / 3)
    # primary_colour: only 2 non-null, both red → winner red, 2/3 of all runs.
    assert consensus["primary_colour"] == "red"
    assert confidence["primary_colour"] == pytest.approx(2 / 3)


def test_quantization_config_builds_4bit_or_none():
    assert quantization_config(None) is None
    cfg = quantization_config("4bit")
    assert cfg.load_in_4bit is True
    with pytest.raises(ValueError, match="quantization"):
        quantization_config("8bit")
