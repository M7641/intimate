"""Wrapper around a Hugging Face VLM (image-text-to-text).

The job of this class is to turn a *generative* model into a *structured
extractor*: we inject the JSON contract derived from the schema into the
prompt, optionally attach an image to the chat template, then parse and
validate the output. The description is required; the image is optional.
"""

from __future__ import annotations

import json
import os
from collections import Counter
from dataclasses import dataclass
from io import BytesIO
from pathlib import Path
from urllib.request import urlopen

import torch
from PIL import Image
from transformers import (
    AutoModelForImageTextToText,
    AutoModelForMultimodalLM,
    AutoProcessor,
)

from extract.schema import ExtractionSchema


@dataclass(frozen=True)
class ModelChoice:
    """One vetted image-text-to-text checkpoint and the hardware it wants.

    ``min_memory_gb`` is the rough device memory needed to load the weights at
    the run dtype (~2 bytes/param at bf16/fp16, plus headroom for activations
    and the KV cache). It guides auto-selection; it is not a hard gate — a model
    can still run below it via CPU offload, just slowly.

    ``loader`` names the transformers auto-class used to load the checkpoint:
    ``"image-text-to-text"`` (Qwen2.5-VL, larger Gemma 4) or ``"multimodal"``
    (the small on-device Gemma 4 E-series). See ``_auto_model``.
    """

    model_id: str
    params: str
    min_memory_gb: float
    note: str
    loader: str = "image-text-to-text"


# Available models, ordered small → large by memory. All accept image content in
# their chat template, from two families: Qwen2.5-VL (Alibaba) and Gemma 4
# (Google, multimodal at every size). The small Gemma 4 E-series are MoE models
# built for on-device use; the 26B-A4B is a Mixture-of-Experts with 4B active
# params (fast, but all 26B of expert weights must still be resident). Gemma
# weights are gated on Hugging Face: accept the licence and `huggingface-cli
# login` first. Pick by key, or let `recommend_model` choose the largest fit.
MODELS: dict[str, ModelChoice] = {
    "qwen2.5-vl-3b": ModelChoice(
        "Qwen/Qwen2.5-VL-3B-Instruct",
        "3B",
        8.0,
        "Laptop / small GPU. ~7 GB at bf16; the default.",
    ),
    "gemma-4-e2b": ModelChoice(
        "google/gemma-4-E2B-it",
        "5.1B (2.3B eff.)",
        12.0,
        "Laptop / on-device. Gated on HF (accept licence + login).",
        loader="multimodal",
    ),
    "gemma-4-e4b": ModelChoice(
        "google/gemma-4-E4B-it",
        "8B (4.5B eff.)",
        16.0,
        "Laptop / small GPU on-device. Gated on HF (accept licence + login).",
        loader="multimodal",
    ),
    "qwen2.5-vl-7b": ModelChoice(
        "Qwen/Qwen2.5-VL-7B-Instruct",
        "7B",
        18.0,
        "Single mid-range GPU (16-24 GB) or a 32 GB Apple Silicon machine.",
    ),
    "gemma-4-26b": ModelChoice(
        "google/gemma-4-26B-A4B-it",
        "26B (4B active)",
        56.0,
        "MoE: fast inference, but loads all 26B. Gated on HF (licence + login).",
    ),
    "gemma-4-31b": ModelChoice(
        "google/gemma-4-31B-it",
        "31B",
        66.0,
        "Dense. 64 GB+ workstation / GPU. Gated on HF (licence + login).",
    ),
    "qwen2.5-vl-32b": ModelChoice(
        "Qwen/Qwen2.5-VL-32B-Instruct",
        "32B",
        72.0,
        "A100/H100-class GPU or a 64-128 GB workstation.",
    ),
    "qwen2.5-vl-72b": ModelChoice(
        "Qwen/Qwen2.5-VL-72B-Instruct",
        "72B",
        160.0,
        "Multi-GPU or a very large unified-memory machine.",
    ),
}

# The smallest model is the safe default everywhere. Kept as a module constant
# (single source of truth) so the CLI and examples agree.
DEFAULT_MODEL = MODELS["qwen2.5-vl-3b"].model_id


def _auto_model(loader: str):
    """Map a ``ModelChoice.loader`` to its transformers auto-class.

    Both classes dispatch on the checkpoint's config; the split exists because
    the small Gemma 4 E-series only register under AutoModelForMultimodalLM.
    Resolved by name at call time so the names stay monkeypatchable in tests.
    """
    if loader == "image-text-to-text":
        return AutoModelForImageTextToText
    if loader == "multimodal":
        return AutoModelForMultimodalLM
    msg = f"unknown loader {loader!r}; use 'image-text-to-text' or 'multimodal'"
    raise ValueError(msg)


def available_memory_gb() -> float:
    """Best-effort device memory ceiling, in GiB.

    CUDA → the selected GPU's total VRAM. MPS / CPU → total system RAM (on Apple
    Silicon that *is* the unified memory the GPU draws on). Used only to pick a
    sensible default model; never a hard limit.
    """
    if torch.cuda.is_available():
        return torch.cuda.get_device_properties(0).total_memory / 1024**3
    try:
        return (os.sysconf("SC_PAGE_SIZE") * os.sysconf("SC_PHYS_PAGES")) / 1024**3
    except (ValueError, OSError, AttributeError):
        return 0.0


def recommend_model(memory_gb: float | None = None) -> str:
    """Return the ``MODELS`` key of the largest checkpoint that fits.

    Falls back to the smallest model when nothing fits (it will still try to
    run, via offload). Pass ``memory_gb`` to override the auto-detected figure.
    """
    budget = available_memory_gb() if memory_gb is None else memory_gb
    fits = [k for k, m in MODELS.items() if m.min_memory_gb <= budget]
    if fits:
        return max(fits, key=lambda k: MODELS[k].min_memory_gb)
    return min(MODELS, key=lambda k: MODELS[k].min_memory_gb)


def resolve(choice: str, *, memory_gb: float | None = None) -> ModelChoice:
    """Resolve a user choice to a full ``ModelChoice`` (id + loader + metadata).

    ``"auto"`` → the recommendation for this machine; a ``MODELS`` key → that
    entry; anything else is treated as a raw Hugging Face id with the default
    image-text-to-text loader (override by adding it to ``MODELS``).
    """
    if choice == "auto":
        return MODELS[recommend_model(memory_gb)]
    if choice in MODELS:
        return MODELS[choice]
    return ModelChoice(choice, "?", 0.0, "Custom Hugging Face id.")


def resolve_model(choice: str, *, memory_gb: float | None = None) -> str:
    """Turn a user choice into a Hugging Face model id (see :func:`resolve`)."""
    return resolve(choice, memory_gb=memory_gb).model_id


_SYSTEM = (
    "You are a product feature extraction engine. You are given a product "
    "description and, when available, an image, then return structured "
    "attributes. Reply with a single valid JSON object only, with no "
    "surrounding text or code fences."
)


def select_device() -> torch.device:
    if torch.cuda.is_available():
        return torch.device("cuda")
    if torch.backends.mps.is_available():
        return torch.device("mps")
    return torch.device("cpu")


def _dtype_for(device: torch.device) -> torch.dtype:
    if device.type == "cuda":
        return torch.bfloat16
    if device.type == "mps":
        return torch.float16
    return torch.float32


def load_image(source: str | Path | Image.Image) -> Image.Image:
    if isinstance(source, Image.Image):
        return source.convert("RGB")
    src = str(source)
    if src.startswith(("http://", "https://")):
        with urlopen(src) as resp:  # noqa: S310 — caller-controlled sources
            data = resp.read()
        return Image.open(BytesIO(data)).convert("RGB")
    return Image.open(src).convert("RGB")


class VisionLanguageExtractor:
    def __init__(
        self,
        model_id: str = DEFAULT_MODEL,
        *,
        device: torch.device | None = None,
        max_new_tokens: int = 512,
        revision: str | None = None,
        quantization: str | None = None,
        constrained: bool = False,
        loader: str = "image-text-to-text",
    ) -> None:
        """Load a VLM checkpoint.

        ``revision`` pins a Hugging Face commit/tag for reproducibility.
        ``quantization="4bit"`` loads bitsandbytes-quantised weights (needs a
        CUDA GPU + ``bitsandbytes``). ``constrained=True`` enables JSON-schema
        guided decoding (needs the optional ``outlines`` package). ``loader``
        picks the transformers auto-class (see ``_auto_model``); take it from a
        registry entry's ``ModelChoice.loader``.
        """
        self.model_id = model_id
        self.revision = revision
        self.max_new_tokens = max_new_tokens
        self.constrained = constrained
        auto_model = _auto_model(loader)
        self.processor = AutoProcessor.from_pretrained(model_id, revision=revision)

        qconfig = _quantization_config(quantization)
        if qconfig is not None:
            # accelerate places the quantised weights; don't move them by hand.
            self.model = auto_model.from_pretrained(
                model_id,
                revision=revision,
                quantization_config=qconfig,
                device_map="auto",
            )
            self.device = self.model.device
            self.dtype = None
        else:
            self.device = device or select_device()
            self.dtype = _dtype_for(self.device)
            self.model = auto_model.from_pretrained(
                model_id, revision=revision, torch_dtype=self.dtype
            ).to(self.device)
        self.model.eval()

    def build_prompt(self, schema: ExtractionSchema, description: str) -> str:
        return (
            f"{schema.prompt_block()}\n\n"
            f"Product description:\n{description.strip() or '(none)'}"
        )

    @torch.inference_mode()
    def extract(
        self,
        schema: ExtractionSchema,
        description: str,
        image: str | Path | Image.Image | None = None,
    ) -> dict:
        """Extract features from a description and an *optional* image.

        The description is the required signal; the image is added to the
        prompt only when provided, so text-only rows extract too.
        """
        return self._generate_one(
            schema, description, image, do_sample=False, temperature=1.0
        )

    @torch.inference_mode()
    def extract_consistent(
        self,
        schema: ExtractionSchema,
        description: str,
        image: str | Path | Image.Image | None = None,
        *,
        samples: int = 5,
        temperature: float = 0.7,
    ) -> tuple[dict, dict]:
        """Sample ``samples`` extractions and majority-vote each field.

        Returns ``(consensus, confidence)`` where ``confidence[field]`` is the
        fraction of samples that agreed on the winning value — a cheap
        abstention signal: low agreement flags a row for human review.
        """
        runs = [
            self._generate_one(
                schema, description, image, do_sample=True, temperature=temperature
            )
            for _ in range(samples)
        ]
        return vote(runs, schema)

    @torch.inference_mode()
    def extract_batch(
        self,
        schema: ExtractionSchema,
        descriptions: list[str],
        images: list[str | Path | Image.Image | None] | None = None,
    ) -> list[dict]:
        """Extract a whole batch in one ``generate()`` call (left-padded).

        Amortises the per-call model overhead — the main throughput lever. The
        batch should be homogeneous (all rows with an image, or all text-only);
        mixing is supported best-effort but pads to the widest input.
        """
        descriptions = list(descriptions)
        images = list(images) if images is not None else [None] * len(descriptions)
        pils = [load_image(im) if im is not None else None for im in images]
        texts = [
            self.processor.apply_chat_template(
                self._messages(schema, d, p), tokenize=False, add_generation_prompt=True
            )
            for d, p in zip(descriptions, pils)
        ]
        proc_kwargs: dict = {"text": texts, "return_tensors": "pt", "padding": True}
        present = [p for p in pils if p is not None]
        if present:
            proc_kwargs["images"] = present
        tok = getattr(self.processor, "tokenizer", None)
        if tok is not None:
            tok.padding_side = "left"  # decoder-only batched generation pads left
        inputs = self.processor(**proc_kwargs).to(self.device)
        generated = self.model.generate(
            **inputs, **self._gen_kwargs(schema, False, 1.0)
        )
        trimmed = generated[:, inputs["input_ids"].shape[1] :]
        decoded = self.processor.batch_decode(trimmed, skip_special_tokens=True)
        return [schema.validate(_parse_json(d)) for d in decoded]

    # --- internals ---------------------------------------------------------

    def _messages(
        self, schema: ExtractionSchema, description: str, pil: Image.Image | None
    ) -> list[dict]:
        content: list[dict] = []
        if pil is not None:
            content.append({"type": "image"})
        content.append({"type": "text", "text": self.build_prompt(schema, description)})
        return [
            {"role": "system", "content": _SYSTEM},
            {"role": "user", "content": content},
        ]

    def _gen_kwargs(
        self, schema: ExtractionSchema, do_sample: bool, temperature: float
    ) -> dict:
        kwargs: dict = {"max_new_tokens": self.max_new_tokens, "do_sample": do_sample}
        if do_sample:
            kwargs["temperature"] = temperature
        processors = self._logits_processors(schema)
        if processors is not None:
            kwargs["logits_processor"] = processors
        return kwargs

    def _logits_processors(self, schema: ExtractionSchema):
        """JSON-schema guided decoding via outlines (optional, untested here)."""
        if not self.constrained:
            return None
        try:
            from outlines.processors import JSONLogitsProcessor
        except ImportError as exc:
            msg = (
                "constrained=True needs the optional `outlines` package for "
                "JSON-schema guided decoding (pip install outlines)"
            )
            raise RuntimeError(msg) from exc
        # API varies across outlines versions; adapt if it changes.
        return [
            JSONLogitsProcessor(
                json.dumps(schema.json_schema()), self.processor.tokenizer
            )
        ]

    def _generate_one(
        self,
        schema: ExtractionSchema,
        description: str,
        image: str | Path | Image.Image | None,
        *,
        do_sample: bool,
        temperature: float,
    ) -> dict:
        pil = load_image(image) if image is not None else None
        messages = self._messages(schema, description, pil)
        text = self.processor.apply_chat_template(
            messages, tokenize=False, add_generation_prompt=True
        )
        proc_kwargs: dict = {"text": [text], "return_tensors": "pt"}
        if pil is not None:
            proc_kwargs["images"] = [pil]
        inputs = self.processor(**proc_kwargs).to(self.device)
        generated = self.model.generate(
            **inputs, **self._gen_kwargs(schema, do_sample, temperature)
        )
        trimmed = generated[:, inputs["input_ids"].shape[1] :]
        decoded = self.processor.batch_decode(trimmed, skip_special_tokens=True)[0]
        return schema.validate(_parse_json(decoded))


def vote(runs: list[dict], schema: ExtractionSchema) -> tuple[dict, dict]:
    """Majority-vote each field across N extraction runs.

    Returns ``(consensus, confidence)``. ``confidence[field]`` is the share of
    runs that agreed on the winning value (0.0 when every run was null).
    """
    n = len(runs) or 1
    consensus: dict[str, object] = {}
    confidence: dict[str, float] = {}
    for col in schema.column_names():
        non_null = [r.get(col) for r in runs if r.get(col) is not None]
        if not non_null:
            consensus[col] = None
            confidence[col] = 0.0
            continue
        value, count = Counter(non_null).most_common(1)[0]
        consensus[col] = value
        confidence[col] = count / n
    return consensus, confidence


def _quantization_config(quantization: str | None):
    if quantization is None:
        return None
    if quantization != "4bit":
        msg = f"unsupported quantization {quantization!r}; use '4bit' or None"
        raise ValueError(msg)
    from transformers import BitsAndBytesConfig

    return BitsAndBytesConfig(
        load_in_4bit=True,
        bnb_4bit_quant_type="nf4",
        bnb_4bit_compute_dtype=torch.float16,
    )


def _parse_json(text: str) -> dict:
    """Pull the first JSON object out of a chatty model output.

    Scans each ``{`` as a candidate start and returns the first that decodes to
    an object via ``raw_decode`` — which stops at the end of the JSON value and
    ignores any trailing prose or code-fence markers. More robust than a greedy
    ``{.*}`` regex, which mis-captures when the output has multiple objects or
    braces in the surrounding chatter.
    """
    decoder = json.JSONDecoder()
    start = text.find("{")
    while start != -1:
        try:
            result, _ = decoder.raw_decode(text, start)
        except json.JSONDecodeError:
            start = text.find("{", start + 1)
            continue
        return result if isinstance(result, dict) else {}
    return {}
