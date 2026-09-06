"""Local HuggingFace LLM wrapper for the ask_warehouse pilot.

A single ``LocalLLM`` instance is loaded once at FastAPI lifespan
startup (~3-5 s, ~3 GB RAM for the default 1.5B FP16 model).
Subsequent requests reuse the in-process model + tokenizer + KV cache.

The interface is deliberately tiny — one ``generate(messages) -> str``
synchronous method, and an async wrapper that hands off to a worker
thread so the FastAPI event loop is never blocked.

Swappable: replace this class with an Anthropic / Ollama backend by
matching the ``.generate`` signature. The rest of the package is
unaware of which inference backend is in use.

Quality vs size: the 1.5B coder loses ~10-15 accuracy points vs the
7B variant on raw text-to-SQL benchmarks, but with our curated
schema + Northwind glossary + few-shot examples grounding most of the
question shape, the gap shrinks substantially. If accuracy on the
golden eval set falls below the 80% bar, the next step is to swap
DEFAULT_MODEL to ``Qwen/Qwen2.5-Coder-3B-Instruct`` (~6 GB FP16) or
its AWQ 4-bit variant (~2 GB but needs ``autoawq``).
"""

from __future__ import annotations

import asyncio
import queue
import threading
import time
from collections.abc import Awaitable, Callable

import structlog
import torch
from transformers import AutoModelForCausalLM, AutoTokenizer, TextIteratorStreamer

log: structlog.stdlib.BoundLogger = structlog.stdlib.get_logger("ask_warehouse.llm")

DEFAULT_MODEL = "Qwen/Qwen2.5-Coder-1.5B-Instruct"


class LocalLLM:
    """Singleton transformers wrapper. Use ``LocalLLM.load(...)`` to construct."""

    def __init__(
        self,
        *,
        model: AutoModelForCausalLM,
        tokenizer: AutoTokenizer,
        model_name: str,
        device: str,
    ) -> None:
        self._model = model
        self._tokenizer = tokenizer
        self.model_name = model_name
        self.device = device

    @classmethod
    def load(
        cls,
        model_name: str = DEFAULT_MODEL,
        *,
        device: str | None = None,
        torch_dtype: torch.dtype = torch.float16,
    ) -> LocalLLM:
        """Load model + tokenizer into memory.

        ``device`` defaults to ``mps`` on Apple Silicon (where MPS is
        available), else ``cuda`` if available, else ``cpu``.
        """
        chosen_device = device or _pick_device()
        t0 = time.perf_counter()
        log.info("ask_warehouse_llm_loading", model=model_name, device=chosen_device)
        tokenizer = AutoTokenizer.from_pretrained(model_name)
        model = AutoModelForCausalLM.from_pretrained(
            model_name,
            torch_dtype=torch_dtype,
            device_map=chosen_device,
        )
        model.eval()
        elapsed = time.perf_counter() - t0
        log.info(
            "ask_warehouse_llm_loaded",
            model=model_name,
            device=chosen_device,
            seconds=round(elapsed, 2),
        )
        return cls(
            model=model,
            tokenizer=tokenizer,
            model_name=model_name,
            device=chosen_device,
        )

    def generate(
        self,
        messages: list[dict[str, str]],
        *,
        max_new_tokens: int = 600,
    ) -> _GenerateResult:
        """Run one deterministic generation. Synchronous — call via ``agenerate``."""
        t0 = time.perf_counter()
        # transformers >=4.45 returns a BatchEncoding (dict-like with
        # ``input_ids`` + ``attention_mask``) here, not a raw tensor. Be
        # explicit so a future default flip doesn't bite us, and pass the
        # mask through to ``generate`` to silence the eos-pad warning.
        inputs = self._tokenizer.apply_chat_template(
            messages,
            add_generation_prompt=True,
            return_tensors="pt",
            return_dict=True,
        ).to(self._model.device)

        input_token_count = int(inputs["input_ids"].shape[-1])

        with torch.inference_mode():
            outputs = self._model.generate(
                **inputs,
                max_new_tokens=max_new_tokens,
                do_sample=False,
                num_beams=1,
                repetition_penalty=1.05,
                temperature=None,
                top_p=None,
                pad_token_id=self._tokenizer.eos_token_id,
            )

        # Only decode the freshly generated tail — strip the prompt echo.
        generated = outputs[0][input_token_count:]
        text = self._tokenizer.decode(generated, skip_special_tokens=True)
        elapsed_ms = int((time.perf_counter() - t0) * 1000)
        return _GenerateResult(
            text=text,
            input_tokens=input_token_count,
            output_tokens=int(generated.shape[-1]),
            duration_ms=elapsed_ms,
        )

    async def agenerate(
        self,
        messages: list[dict[str, str]],
        *,
        max_new_tokens: int = 600,
    ) -> _GenerateResult:
        """Async wrapper — offloads ``generate`` to a worker thread."""
        return await asyncio.to_thread(
            self.generate, messages, max_new_tokens=max_new_tokens
        )

    async def astream(
        self,
        messages: list[dict[str, str]],
        *,
        on_chunk: Callable[[str], Awaitable[None]],
        on_status: Callable[[str], Awaitable[None]] | None = None,
        max_new_tokens: int = 600,
    ) -> _GenerateResult:
        """Stream decoded text chunks to ``on_chunk`` as the model emits them.

        Pattern: HuggingFace ``TextIteratorStreamer`` plus a background
        thread running ``model.generate``. The streamer is a *blocking*
        Python iterator (each ``next()`` waits for the next token batch),
        so we delegate each ``next()`` to a worker thread via
        ``run_in_executor`` and keep the event loop responsive between
        chunks. After the iterator drains we re-tokenise the accumulated
        text to recover ``output_tokens`` for parity with ``generate``.
        """
        t0 = time.perf_counter()
        log.info("ask_warehouse_astream_start", device=self.device)
        if on_status is not None:
            await on_status("tokenising prompt")
        inputs = self._tokenizer.apply_chat_template(
            messages,
            add_generation_prompt=True,
            return_tensors="pt",
            return_dict=True,
        ).to(self._model.device)
        input_token_count = int(inputs["input_ids"].shape[-1])
        log.info(
            "ask_warehouse_astream_prompt_tokenised",
            input_tokens=input_token_count,
            elapsed_ms=int((time.perf_counter() - t0) * 1000),
        )

        # ``timeout`` is the per-``next()`` wait on the streamer's internal
        # queue: if the generate thread dies silently, the iterator would
        # otherwise block forever waiting for tokens that never arrive.
        # 60 s comfortably accommodates first-token latency on MPS for
        # the 1.5B model and still surfaces hangs in a reasonable time.
        streamer = TextIteratorStreamer(
            self._tokenizer,
            skip_prompt=True,
            skip_special_tokens=True,
            timeout=60.0,
        )

        # The producer thread runs ``model.generate`` synchronously. Any
        # exception it raises would otherwise be swallowed (threading.Thread
        # has no return channel); we capture it here and re-raise from the
        # consumer side after the iterator drains or times out.
        thread_error: list[BaseException] = []

        def _run_generate() -> None:
            try:
                with torch.inference_mode():
                    self._model.generate(
                        **inputs,
                        streamer=streamer,
                        max_new_tokens=max_new_tokens,
                        do_sample=False,
                        num_beams=1,
                        repetition_penalty=1.05,
                        temperature=None,
                        top_p=None,
                        pad_token_id=self._tokenizer.eos_token_id,
                    )
            except BaseException as exc:  # noqa: BLE001 — re-raised on consumer
                log.exception("ask_warehouse_llm_generate_thread_failed")
                thread_error.append(exc)
                # Wake the consumer if it's blocked on the queue: the
                # streamer's ``end`` sentinel is what ``__next__`` checks
                # for to raise StopIteration.
                streamer.end()

        thread = threading.Thread(target=_run_generate, daemon=True)
        thread.start()
        log.info(
            "ask_warehouse_astream_generate_started", max_new_tokens=max_new_tokens
        )
        if on_status is not None:
            await on_status(
                f"model thinking ({input_token_count} input tokens, "
                f"device={self.device})"
            )

        loop = asyncio.get_running_loop()
        sentinel: object = object()
        parts: list[str] = []
        first_chunk_logged = False
        while True:
            try:
                chunk = await loop.run_in_executor(
                    None, lambda: next(streamer, sentinel)
                )
            except queue.Empty as exc:
                # ``next`` waited ``timeout`` seconds and got nothing — the
                # generate thread is either dead or stuck. Prefer the
                # original exception if we captured one.
                if thread_error:
                    raise thread_error[0] from None
                raise RuntimeError(
                    "LLM generation stalled (no tokens for 60 s)"
                ) from exc
            if chunk is sentinel:
                break
            if not first_chunk_logged:
                log.info(
                    "ask_warehouse_astream_first_chunk",
                    elapsed_ms=int((time.perf_counter() - t0) * 1000),
                    chunk_len=len(chunk),  # type: ignore[arg-type]
                )
                first_chunk_logged = True
            parts.append(chunk)  # type: ignore[arg-type]
            await on_chunk(chunk)  # type: ignore[arg-type]

        # The producer thread has emitted its last chunk and exited by the
        # time the streamer signals StopIteration; join to surface any
        # exception it raised inside ``model.generate``.
        await asyncio.to_thread(thread.join)
        if thread_error:
            raise thread_error[0]

        text = "".join(parts)
        output_tokens = len(self._tokenizer.encode(text, add_special_tokens=False))
        elapsed_ms = int((time.perf_counter() - t0) * 1000)
        return _GenerateResult(
            text=text,
            input_tokens=input_token_count,
            output_tokens=output_tokens,
            duration_ms=elapsed_ms,
        )


class _GenerateResult:
    __slots__ = ("text", "input_tokens", "output_tokens", "duration_ms")

    def __init__(
        self,
        *,
        text: str,
        input_tokens: int,
        output_tokens: int,
        duration_ms: int,
    ) -> None:
        self.text = text
        self.input_tokens = input_tokens
        self.output_tokens = output_tokens
        self.duration_ms = duration_ms


def _pick_device() -> str:
    if torch.backends.mps.is_available():
        return "mps"
    if torch.cuda.is_available():
        return "cuda"
    return "cpu"
