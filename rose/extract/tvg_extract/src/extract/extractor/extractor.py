"""Wrapper around a Hugging Face text-to-text model.

The job of this class is to turn a *generative* model into a *structured
extractor*: we inject the JSON contract derived from the schema into the prompt,
run it through the model's chat template, then parse and validate the output.
Text in, structured dict out.
"""

from __future__ import annotations

import json
import logging
import os
from collections import Counter
from dataclasses import dataclass

import torch
from transformers import AutoModelForCausalLM, AutoTokenizer, BitsAndBytesConfig

from extract.schema import ExtractionSchema

logger = logging.getLogger(__name__)


@dataclass(frozen=True)
class ModelChoice:
    """One vetted checkpoint and the hardware it wants.

    ``min_memory_gb`` is the rough device memory needed to load the weights at
    the run dtype (~2 bytes/param at bf16/fp16, plus headroom for activations
    and the KV cache). It guides auto-selection; it is not a hard gate — a model
    can still run below it via CPU offload, just slowly.
    """

    model_id: str
    params: str
    min_memory_gb: float
    note: str


# A clean ladder of text-to-text models (Qwen2.5 Instruct), ordered small →
# large by memory, so a job can target anything from a laptop to a multi-GPU
# box. Pick by key, or let ``recommend_model`` choose the largest fit.
MODELS: dict[str, ModelChoice] = {
    "qwen2.5-0.5b": ModelChoice(
        "Qwen/Qwen2.5-0.5B-Instruct",
        "0.5B",
        2.0,
        "Tiny. Laptop / CPU. Fastest; lowest quality.",
    ),
    "qwen2.5-1.5b": ModelChoice(
        "Qwen/Qwen2.5-1.5B-Instruct",
        "1.5B",
        4.0,
        "Small. Laptop. Reliable JSON; the hierarchy default.",
    ),
    "qwen2.5-3b": ModelChoice(
        "Qwen/Qwen2.5-3B-Instruct",
        "3B",
        8.0,
        "Small GPU or a capable laptop.",
    ),
    "qwen2.5-7b": ModelChoice(
        "Qwen/Qwen2.5-7B-Instruct",
        "7B",
        18.0,
        "Single mid-range GPU (16-24 GB) or a 32 GB Apple Silicon machine.",
    ),
    "qwen2.5-14b": ModelChoice(
        "Qwen/Qwen2.5-14B-Instruct",
        "14B",
        32.0,
        "Single larger GPU (24-48 GB).",
    ),
    "qwen2.5-32b": ModelChoice(
        "Qwen/Qwen2.5-32B-Instruct",
        "32B",
        72.0,
        "A100/H100-class GPU or a 64-128 GB workstation.",
    ),
    "qwen2.5-72b": ModelChoice(
        "Qwen/Qwen2.5-72B-Instruct",
        "72B",
        160.0,
        "Multi-GPU or a very large unified-memory machine.",
    ),
}

# The smallest model is the safe default everywhere. Kept as a module constant
# (single source of truth) so the CLI and examples agree.
DEFAULT_MODEL = MODELS["qwen2.5-0.5b"].model_id

# The instruction that frames every extraction.
SYSTEM_PROMPT = (
    "You are a product feature extraction engine. You are given a product "
    "description and return structured attributes. Reply with a single valid "
    "JSON object only, with no surrounding text or code fences."
)


def build_prompt(schema: ExtractionSchema, description: str) -> str:
    """Render the user-turn text: the schema's contract + the description.

    Pure — the JSON contract comes from the schema, the description is the
    only variable.
    """
    return (
        f"{schema.prompt_block()}\n\n"
        f"Product description:\n{description.strip() or '(none)'}"
    )


def build_messages(schema: ExtractionSchema, description: str) -> list[dict]:
    """The full chat conversation (system + user) for one description."""
    return [
        {"role": "system", "content": SYSTEM_PROMPT},
        {"role": "user", "content": build_prompt(schema, description)},
    ]


# The grouped (many-products-per-prompt) variant of SYSTEM_PROMPT — the C3
# experiment in README. The reply contract is an array, not an object.
GROUPED_SYSTEM_PROMPT = (
    "You are a product feature extraction engine. You are given several "
    "numbered product descriptions and return structured attributes for each. "
    "Reply with a single valid JSON array only — one object per description, "
    "in the same order — with no surrounding text or code fences."
)


def build_grouped_prompt(schema: ExtractionSchema, descriptions: list[str]) -> str:
    """Render one user turn covering several descriptions (C3 prompt batching).

    The schema contract appears once, so its prompt cost is amortised across
    the whole group. Descriptions are numbered and the reply must be an array
    of exactly ``len(descriptions)`` objects in the same order — the count is
    what lets the caller detect (and recover from) a misaligned reply.
    """
    listing = "\n".join(
        f"{i + 1}. {d.strip() or '(none)'}" for i, d in enumerate(descriptions)
    )
    return (
        f"{schema.prompt_block()}\n\n"
        f"Extract the attributes for each of the {len(descriptions)} product "
        f"descriptions below. Return a JSON array with exactly "
        f"{len(descriptions)} objects, one per description, in the same "
        f"order.\n\n"
        f"Product descriptions:\n{listing}"
    )


def build_grouped_messages(
    schema: ExtractionSchema, descriptions: list[str]
) -> list[dict]:
    """The full chat conversation for one grouped (multi-product) prompt."""
    return [
        {"role": "system", "content": GROUPED_SYSTEM_PROMPT},
        {"role": "user", "content": build_grouped_prompt(schema, descriptions)},
    ]


def grouped_json_schema(schema: ExtractionSchema, n: int) -> dict:
    """JSON Schema for a grouped reply: an array of exactly ``n`` objects.

    This is what constrained decoding enforces in grouped mode — the same
    per-object contract as ``schema.json_schema()``, pinned to the group size
    so the model cannot drop or invent rows.
    """
    return {
        "type": "array",
        "items": schema.json_schema(),
        "minItems": n,
        "maxItems": n,
    }


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
    """Resolve a user choice to a full ``ModelChoice`` (id + metadata).

    ``"auto"`` → the recommendation for this machine; a ``MODELS`` key → that
    entry; anything else is treated as a raw Hugging Face id.
    """
    if choice == "auto":
        return MODELS[recommend_model(memory_gb)]
    if choice in MODELS:
        return MODELS[choice]
    return ModelChoice(choice, "?", 0.0, "Custom Hugging Face id.")


def resolve_model(choice: str, *, memory_gb: float | None = None) -> str:
    """Turn a user choice into a Hugging Face model id (see :func:`resolve`)."""
    return resolve(choice, memory_gb=memory_gb).model_id


def select_device() -> torch.device:
    if torch.cuda.is_available():
        return torch.device("cuda")
    if torch.backends.mps.is_available():
        return torch.device("mps")
    return torch.device("cpu")


def dtype_for(device: torch.device) -> torch.dtype:
    if device.type == "cuda":
        return torch.bfloat16
    if device.type == "mps":
        return torch.float16
    return torch.float32


class TextExtractor:
    def __init__(
        self,
        model_id: str = DEFAULT_MODEL,
        *,
        device: torch.device | None = None,
        max_new_tokens: int = 512,
        revision: str | None = None,
        quantization: str | None = None,
        constrained: bool = False,
    ) -> None:
        """Load a text-to-text checkpoint (a causal LM + its tokenizer).

        ``revision`` pins a Hugging Face commit/tag for reproducibility.
        ``quantization="4bit"`` loads bitsandbytes-quantised weights (needs a
        CUDA GPU + ``bitsandbytes``). ``constrained=True`` enables JSON-schema
        guided decoding (needs the optional ``outlines`` package).
        """
        self.model_id = model_id
        self.revision = revision
        self.max_new_tokens = max_new_tokens
        self.constrained = constrained
        self.tokenizer = AutoTokenizer.from_pretrained(model_id, revision=revision)

        qconfig = quantization_config(quantization)
        if qconfig is not None:
            # accelerate places the quantised weights; don't move them by hand.
            self.model = AutoModelForCausalLM.from_pretrained(
                model_id,
                revision=revision,
                quantization_config=qconfig,
                device_map="auto",
            )
            self.device = self.model.device
            self.dtype = None
        else:
            self.device = device or select_device()
            self.dtype = dtype_for(self.device)
            self.model = AutoModelForCausalLM.from_pretrained(
                model_id, revision=revision, torch_dtype=self.dtype
            ).to(self.device)
        self.model.eval()

    def build_prompt(self, schema: ExtractionSchema, description: str) -> str:
        return build_prompt(schema, description)

    @torch.inference_mode()
    def extract(self, schema: ExtractionSchema, description: str) -> dict:
        """Extract structured features from a product description."""
        return self.generate_one(schema, description, do_sample=False, temperature=1.0)

    @torch.inference_mode()
    def extract_consistent(
        self,
        schema: ExtractionSchema,
        description: str,
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
            self.generate_one(
                schema, description, do_sample=True, temperature=temperature
            )
            for _ in range(samples)
        ]
        return vote(runs, schema)

    @torch.inference_mode()
    def extract_batch(
        self, schema: ExtractionSchema, descriptions: list[str]
    ) -> list[dict]:
        """Extract a whole batch in one ``generate()`` call (left-padded).

        Amortises the per-call model overhead — the main throughput lever. A
        failed batch is the caller's concern; each output is parsed/validated
        independently.
        """
        texts = [
            self.tokenizer.apply_chat_template(
                self.messages(schema, d), tokenize=False, add_generation_prompt=True
            )
            for d in descriptions
        ]
        self.tokenizer.padding_side = "left"  # decoder-only batches pad left
        # Some checkpoints ship no pad token; fall back to EOS (HF-recommended)
        # so padding=True can form the batch.
        if (
            getattr(self.tokenizer, "pad_token_id", None) is None
            and getattr(self.tokenizer, "eos_token", None) is not None
        ):
            self.tokenizer.pad_token = self.tokenizer.eos_token
        inputs = self.tokenizer(texts, return_tensors="pt", padding=True).to(
            self.device
        )
        generated = self.model.generate(
            **inputs, **self.gen_kwargs(schema.json_schema(), False, 1.0)
        )
        trimmed = generated[:, inputs["input_ids"].shape[1] :]
        decoded = self.tokenizer.batch_decode(trimmed, skip_special_tokens=True)
        return [schema.validate(parse_json(d)) for d in decoded]

    @torch.inference_mode()
    def extract_grouped(
        self, schema: ExtractionSchema, descriptions: list[str]
    ) -> list[dict]:
        """Extract several descriptions from ONE prompt (C3 prompt batching).

        The schema contract is paid once per group instead of once per row. A
        misaligned reply (wrong count, non-object rows, no array) falls back to
        one per-row :meth:`extract` per description, so a bad group degrades to
        the slow path rather than nulling its rows.
        """
        kwargs = self.gen_kwargs(
            grouped_json_schema(schema, len(descriptions)), False, 1.0
        )
        # The reply carries one object per description; budget tokens to match.
        kwargs["max_new_tokens"] = self.max_new_tokens * len(descriptions)
        decoded = self.generate_text(
            build_grouped_messages(schema, descriptions), kwargs
        )
        return parse_grouped_reply(self, schema, descriptions, decoded)

    # --- internals ---------------------------------------------------------

    def messages(self, schema: ExtractionSchema, description: str) -> list[dict]:
        return build_messages(schema, description)

    def gen_kwargs(
        self, json_schema: dict, do_sample: bool, temperature: float
    ) -> dict:
        kwargs: dict = {"max_new_tokens": self.max_new_tokens, "do_sample": do_sample}
        if do_sample:
            kwargs["temperature"] = temperature
        processors = self.logits_processors(json_schema)
        if processors is not None:
            kwargs["logits_processor"] = processors
        return kwargs

    def logits_processors(self, json_schema: dict):
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
        return [JSONLogitsProcessor(json.dumps(json_schema), self.tokenizer)]

    def generate_text(self, messages: list[dict], gen_kwargs: dict) -> str:
        """One templated chat call → the decoded completion text."""
        text = self.tokenizer.apply_chat_template(
            messages, tokenize=False, add_generation_prompt=True
        )
        inputs = self.tokenizer([text], return_tensors="pt").to(self.device)
        generated = self.model.generate(**inputs, **gen_kwargs)
        trimmed = generated[:, inputs["input_ids"].shape[1] :]
        return self.tokenizer.batch_decode(trimmed, skip_special_tokens=True)[0]

    def generate_one(
        self,
        schema: ExtractionSchema,
        description: str,
        *,
        do_sample: bool,
        temperature: float,
    ) -> dict:
        decoded = self.generate_text(
            self.messages(schema, description),
            self.gen_kwargs(schema.json_schema(), do_sample, temperature),
        )
        return schema.validate(parse_json(decoded))


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


def quantization_config(quantization: str | None):
    if quantization is None:
        return None
    if quantization != "4bit":
        msg = f"unsupported quantization {quantization!r}; use '4bit' or None"
        raise ValueError(msg)

    return BitsAndBytesConfig(
        load_in_4bit=True,
        bnb_4bit_quant_type="nf4",
        bnb_4bit_compute_dtype=torch.float16,
    )


def parse_json(text: str) -> dict:
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


def parse_json_array(text: str) -> list:
    """Pull the first JSON array out of a chatty model output.

    The grouped-prompt counterpart of :func:`parse_json` — same scanning
    strategy, anchored on ``[`` and requiring a top-level list. Returns ``[]``
    when nothing decodes to an array.
    """
    decoder = json.JSONDecoder()
    start = text.find("[")
    while start != -1:
        try:
            result, _ = decoder.raw_decode(text, start)
        except json.JSONDecodeError:
            start = text.find("[", start + 1)
            continue
        return result if isinstance(result, list) else []
    return []


def parse_grouped_reply(
    extractor, schema: ExtractionSchema, descriptions: list[str], text: str
) -> list[dict]:
    """Validate a grouped reply, falling back to per-row extraction.

    A grouped generation fails as a whole — no array, wrong count, non-object
    rows — and one bad reply must not null its whole group. Any misalignment
    re-extracts each description individually via ``extractor.extract``,
    trading the amortised speed back for per-row reliability.
    """
    items = parse_json_array(text)
    if len(items) == len(descriptions) and all(isinstance(i, dict) for i in items):
        return [schema.validate(item) for item in items]
    logger.warning(
        "grouped reply misaligned (%d items for %d descriptions); "
        "falling back to per-row extraction",
        len(items),
        len(descriptions),
    )
    return [extractor.extract(schema, d) for d in descriptions]
