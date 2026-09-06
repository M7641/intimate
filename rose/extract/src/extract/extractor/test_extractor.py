"""The model-free helpers in the extractor: JSON parsing, dtype, image load.

The model itself isn't loaded here — only the pure pieces around it. The
chatty-output JSON parser is the one most worth pinning.
"""

from __future__ import annotations

from types import SimpleNamespace

import pytest
import torch
from PIL import Image

import extract.extractor.extractor as em
from extract.domains import get_schema
from extract.extractor.extractor import (
    VisionLanguageExtractor,
    _dtype_for,
    _parse_json,
    _quantization_config,
    load_image,
    vote,
)


def test_parse_json_extracts_a_clean_object():
    assert _parse_json('{"a": 1}') == {"a": 1}


def test_parse_json_ignores_prose_and_code_fences():
    text = 'Sure!\n```json\n{"a": 1, "b": "x"}\n```\nHope that helps.'
    assert _parse_json(text) == {"a": 1, "b": "x"}


def test_parse_json_returns_empty_on_non_json():
    assert _parse_json("there is no json here") == {}


def test_parse_json_rejects_top_level_non_object():
    # A bare array is valid JSON but not the dict the schema expects.
    assert _parse_json("[1, 2, 3]") == {}


def test_parse_json_stops_at_end_of_object_ignoring_trailing_prose():
    assert _parse_json('{"a": 1} and that is my final answer }') == {"a": 1}


def test_parse_json_returns_first_object_when_several():
    assert _parse_json('{"a": 1} {"b": 2}') == {"a": 1}


def test_parse_json_skips_a_false_start_brace():
    # The first "{" does not begin valid JSON; scanning finds the real object.
    assert _parse_json('prefix {not json} {"a": 1}') == {"a": 1}


def test_dtype_for_cpu_is_float32():
    assert _dtype_for(torch.device("cpu")) == torch.float32


def test_load_image_converts_pil_to_rgb():
    assert load_image(Image.new("L", (4, 4))).mode == "RGB"


def test_load_image_from_local_path(tmp_path):
    path = tmp_path / "x.png"
    Image.new("RGB", (4, 4), "red").save(path)
    out = load_image(str(path))
    assert out.size == (4, 4)
    assert out.mode == "RGB"


# --- extract() orchestration, with the model/processor mocked (no weights) ---


class _FakeBatch(dict):
    def to(self, device):  # the real processor output exposes .to(device)
        return self


class _FakeProcessor:
    def __init__(self):
        self.tokenizer = SimpleNamespace(padding_side="right")

    def apply_chat_template(self, messages, tokenize, add_generation_prompt):
        self.last_messages = messages
        return "PROMPT"

    def __call__(self, text, return_tensors, images=None, padding=False):
        self.last_images = images
        # One row per text → exercises both single and batched paths.
        return _FakeBatch(input_ids=torch.zeros((len(text), 3), dtype=torch.long))

    def batch_decode(self, trimmed, skip_special_tokens):
        # Chatty output, one per row; the parser must still recover each object.
        return [
            'Sure: {"category": "dress", "primary_colour": "red"} ✅'
        ] * trimmed.shape[0]


class _FakeModel:
    def to(self, device):
        return self

    def eval(self):
        return self

    def generate(self, input_ids=None, **kw):
        n = input_ids.shape[0] if input_ids is not None else 1
        return torch.zeros((n, 5), dtype=torch.long)  # 3 prompt + 2 generated


def _build(monkeypatch, **kwargs) -> VisionLanguageExtractor:
    monkeypatch.setattr(
        em,
        "AutoProcessor",
        type(
            "P", (), {"from_pretrained": staticmethod(lambda *a, **k: _FakeProcessor())}
        ),
    )
    monkeypatch.setattr(
        em,
        "AutoModelForImageTextToText",
        type("M", (), {"from_pretrained": staticmethod(lambda *a, **k: _FakeModel())}),
    )
    return VisionLanguageExtractor("fake-model", device=torch.device("cpu"), **kwargs)


def test_extract_text_only_omits_image_block(monkeypatch):
    ex = _build(monkeypatch)
    result = ex.extract(get_schema("clothing"), "a red maxi dress")  # no image

    # The user message carries no image part, and no images were sent.
    user_content = ex.processor.last_messages[1]["content"]
    assert all(part["type"] != "image" for part in user_content)
    assert ex.processor.last_images is None
    # The chatty JSON was parsed and validated through the schema.
    assert result["category"] == "dress"
    assert result["primary_colour"] == "red"


def test_extract_with_image_attaches_image_block(monkeypatch):
    ex = _build(monkeypatch)
    ex.extract(get_schema("clothing"), "x", Image.new("RGB", (4, 4)))

    user_content = ex.processor.last_messages[1]["content"]
    assert any(part["type"] == "image" for part in user_content)
    assert ex.processor.last_images is not None


def test_extract_batch_returns_one_dict_per_row(monkeypatch):
    ex = _build(monkeypatch)
    out = ex.extract_batch(get_schema("clothing"), ["a red dress", "a top"])
    assert len(out) == 2
    assert all(row["category"] == "dress" for row in out)
    # Decoder-only batched generation must left-pad.
    assert ex.processor.tokenizer.padding_side == "left"


def test_extract_consistent_votes_and_scores_confidence(monkeypatch):
    ex = _build(monkeypatch)
    consensus, confidence = ex.extract_consistent(
        get_schema("clothing"), "a red dress", samples=3
    )
    assert consensus["category"] == "dress"
    # All three (identical) samples agree.
    assert confidence["category"] == 1.0


def test_constrained_without_outlines_raises_clear_error(monkeypatch):
    ex = _build(monkeypatch, constrained=True)
    with pytest.raises(RuntimeError, match="outlines"):
        ex.extract(get_schema("clothing"), "a red dress")


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
    assert _quantization_config(None) is None
    cfg = _quantization_config("4bit")
    assert cfg.load_in_4bit is True
    with pytest.raises(ValueError, match="quantization"):
        _quantization_config("8bit")
