"""Contrastive training loop."""

from __future__ import annotations

import time
from collections.abc import Callable
from dataclasses import dataclass, field
from typing import Literal

import torch
from torch import Tensor
from torch.amp import GradScaler, autocast
from torch.optim.lr_scheduler import (
    CosineAnnealingLR,
    LinearLR,
    SequentialLR,
)
from torch.utils.data import DataLoader

from .augmentations import CurriculumAugmentation
from .data import SetBatch
from .ema import EMAEncoder
from .methods.base import BaseMethod

EpochCallback = Callable[[int, int, float, float], None]
"""(epoch, total_epochs, avg_loss, lr) -> None"""

# -- ANSI helpers for epoch display --
_DIM = "\033[90m"
_CYAN_B = "\033[96;1m"
_GREEN = "\033[32m"
_YELLOW = "\033[33m"
_RED = "\033[31m"
_RST = "\033[0m"


def _loss_bar(loss: float, max_loss: float, width: int = 24) -> str:
    fill = max(1, int(round(loss / max_loss * width)))
    if loss < max_loss * 0.4:
        c = _GREEN
    elif loss < max_loss * 0.7:
        c = _YELLOW
    else:
        c = _RED
    return f"{c}{'━' * fill}{_DIM}{'╌' * (width - fill)}{_RST}"


def _fmt_duration(seconds: float) -> str:
    if seconds < 1.0:
        return f"{seconds * 1000:.0f}ms"
    if seconds < 60.0:
        return f"{seconds:.1f}s"
    m, s = divmod(seconds, 60)
    return f"{int(m)}m{s:04.1f}s"


def _print_epoch(
    epoch: int,
    total: int,
    loss: float,
    max_loss: float,
    lr: float,
    elapsed: float,
) -> None:
    pad = len(str(total))
    tag = f"{_DIM}  epoch {epoch + 1:>{pad}}/{total}{_RST}"
    bar = _loss_bar(loss, max_loss)
    loss_s = f"{_CYAN_B}{loss:.4f}{_RST}"
    lr_s = f"{_DIM}lr={lr:.1e}{_RST}"
    time_s = f"{_DIM}{_fmt_duration(elapsed)}{_RST}"
    print(f"{tag}  {bar}  {loss_s}  {lr_s}  {time_s}", flush=True)


@dataclass
class TrainConfig:
    """Configuration for contrastive training."""

    epochs: int = 50
    # TUNING: lr interacts strongly with batch_size. Larger batches tolerate
    # higher lr (linear scaling rule: lr ~ base_lr * batch_size/256).
    # Start at 3e-4 for batch_size=32, try 1e-3 for batch_size>=128.
    # If loss oscillates wildly, halve it. If loss plateaus early, double it.
    lr: float = 3e-4
    # TUNING: weight_decay regularises all parameters via L2 penalty.
    # Increase to 1e-4 or 1e-3 for small datasets to prevent overfitting.
    # Set to 0 if you already use strong augmentations as regularisation.
    weight_decay: float = 1e-5
    max_grad_norm: float | None = None
    device: str = "cpu"
    # TUNING: warmup prevents early instability from large initial gradients.
    # Use 5-10% of total epochs (e.g. 5 for 100 epochs). Most useful when
    # combined with cosine scheduling and higher learning rates.
    warmup_epochs: int = 0
    scheduler: Literal["none", "cosine"] = "none"
    verbose: bool = True
    patience: int | None = None
    amp: bool = False


@dataclass
class TrainResult:
    """Result of a training run."""

    epoch_losses: list[float] = field(default_factory=list)
    epoch_lrs: list[float] = field(default_factory=list)
    val_losses: list[float] = field(default_factory=list)
    stopped_early: bool = False

    @property
    def final_loss(self) -> float:
        return self.epoch_losses[-1] if self.epoch_losses else float("inf")


def _build_scheduler(
    optimizer: torch.optim.Optimizer,
    config: TrainConfig,
) -> torch.optim.lr_scheduler.LRScheduler | None:
    """Build LR scheduler from config. Returns None if no scheduling requested."""
    if config.scheduler == "none" and config.warmup_epochs == 0:
        return None

    remaining = max(config.epochs - config.warmup_epochs, 1)

    # Main scheduler (post-warmup)
    if config.scheduler == "cosine":
        main = CosineAnnealingLR(optimizer, T_max=remaining)
    else:
        main = None

    # Warmup
    if config.warmup_epochs > 0:
        warmup = LinearLR(
            optimizer,
            start_factor=0.01,
            end_factor=1.0,
            total_iters=config.warmup_epochs,
        )
        if main is not None:
            return SequentialLR(
                optimizer, schedulers=[warmup, main], milestones=[config.warmup_epochs]
            )
        return warmup

    return main


def _find_curriculum(model: BaseMethod) -> list[CurriculumAugmentation]:
    """Find all CurriculumAugmentation modules in the method."""
    return [m for m in model.modules() if isinstance(m, CurriculumAugmentation)]


def _find_ema(model: BaseMethod) -> EMAEncoder | None:
    """Find the EMAEncoder in the method, if any."""
    for m in model.modules():
        if isinstance(m, EMAEncoder):
            return m
    return None


class SSLTrainer:
    """Trains any self-supervised method (axis B) — contrastive, masked, etc.

    Method-agnostic: it only calls `method.training_step(x, mask)` and reuses
    the same optimiser, scheduler, curriculum, EMA, early-stopping, and AMP
    machinery for every objective.
    """

    def __init__(self, model: BaseMethod, config: TrainConfig | None = None) -> None:
        self.model = model
        self.config = config or TrainConfig()

    def fit(
        self,
        dataloader: DataLoader[SetBatch],
        epoch_callback: EpochCallback | None = None,
        val_dataloader: DataLoader[SetBatch] | None = None,
    ) -> TrainResult:
        config = self.config
        device = torch.device(config.device)
        self.model.to(device)
        self.model.train()

        optimizer = torch.optim.Adam(
            self.model.parameters(), lr=config.lr, weight_decay=config.weight_decay
        )

        scheduler = _build_scheduler(optimizer, config)
        curricula = _find_curriculum(self.model)
        ema = _find_ema(self.model)
        scaler = GradScaler(enabled=config.amp and device.type == "cuda")

        result = TrainResult()
        first_loss: float | None = None

        # Early stopping state
        best_loss = float("inf")
        best_state: dict[str, Tensor] | None = None
        patience_counter = 0

        for epoch in range(config.epochs):
            epoch_start = time.perf_counter()

            # Update curriculum and EMA progress
            progress = epoch / max(config.epochs - 1, 1)
            for cur in curricula:
                cur.set_progress(progress)
            if ema is not None:
                ema.set_progress(progress)

            total_loss = 0.0
            num_batches = 0

            for batch in dataloader:
                x = batch.x.to(device)
                mask = batch.mask.to(device)

                with autocast(device_type=device.type, enabled=config.amp):
                    loss = self.model.training_step(x, mask)

                optimizer.zero_grad()
                scaler.scale(loss).backward()

                if config.max_grad_norm is not None:
                    scaler.unscale_(optimizer)
                    torch.nn.utils.clip_grad_norm_(
                        self.model.parameters(), config.max_grad_norm
                    )

                scaler.step(optimizer)
                scaler.update()

                if ema is not None:
                    ema.update(self.model.encoder)

                total_loss += loss.item()
                num_batches += 1

            if scheduler is not None:
                scheduler.step()

            avg_loss = total_loss / max(num_batches, 1)
            current_lr = optimizer.param_groups[0]["lr"]
            result.epoch_losses.append(avg_loss)
            result.epoch_lrs.append(current_lr)

            if first_loss is None:
                first_loss = avg_loss

            # Validation pass
            val_loss: float | None = None
            if val_dataloader is not None:
                self.model.set_eval_mode()
                val_total = 0.0
                val_batches = 0
                with torch.no_grad():
                    for val_batch in val_dataloader:
                        vx = val_batch.x.to(device)
                        vm = val_batch.mask.to(device)
                        with autocast(device_type=device.type, enabled=config.amp):
                            vl = self.model.training_step(vx, vm)
                        val_total += vl.item()
                        val_batches += 1
                val_loss = val_total / max(val_batches, 1)
                result.val_losses.append(val_loss)
                self.model.train()

            epoch_elapsed = time.perf_counter() - epoch_start

            if config.verbose:
                _print_epoch(
                    epoch,
                    config.epochs,
                    avg_loss,
                    first_loss,
                    current_lr,
                    epoch_elapsed,
                )
                if val_loss is not None:
                    print(f"  {_DIM}  val_loss={val_loss:.4f}{_RST}", flush=True)

            if epoch_callback is not None:
                epoch_callback(epoch, config.epochs, avg_loss, current_lr)

            # Early stopping — prefer val_loss when available
            stopping_loss = val_loss if val_loss is not None else avg_loss
            if config.patience is not None:
                if stopping_loss < best_loss:
                    best_loss = stopping_loss
                    best_state = {
                        k: v.clone() for k, v in self.model.state_dict().items()
                    }
                    patience_counter = 0
                else:
                    patience_counter += 1
                    if patience_counter >= config.patience:
                        if config.verbose:
                            print(
                                f"  Early stopping at epoch {epoch + 1} (patience={config.patience})",
                                flush=True,
                            )
                        result.stopped_early = True
                        break

        if best_state is not None:
            self.model.load_state_dict(best_state)

        return result


# Backward-compatible alias: the trainer is no longer contrastive-specific.
ContrastiveTrainer = SSLTrainer
