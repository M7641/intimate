# Training Infrastructure: LR Scheduling and Warmup

## The Training Loop

Mimic's `ContrastiveTrainer` handles the training loop:

```python
config = mimic.TrainConfig(epochs=50, lr=3e-4)
result = mimic.ContrastiveTrainer(model, config).fit(dataloader)
```

But behind this simple API, several important mechanisms are at work.

---

## Learning Rate Scheduling

### Why Not Just Use a Fixed Learning Rate?

A fixed learning rate is a compromise:
- **Too high**: The model overshoots and oscillates around the minimum
- **Too low**: Training is painfully slow and may get stuck in shallow local minima

The solution: **start high** (to make rapid progress) and **decay** (to converge precisely).

### Warmup: Don't Sprint at the Starting Line

At the very beginning of training, the model's weights are random. The gradients are noisy and potentially very large. A high learning rate + large gradients = parameter explosion.

**Linear warmup** starts at a tiny learning rate (1% of the target) and linearly increases to the full rate over the first few epochs:

```
lr(epoch) = base_lr × epoch / warmup_epochs    (during warmup)
```

```python
config = mimic.TrainConfig(
    lr=3e-4,
    warmup_epochs=5,  # Linear warmup for 5 epochs
)
```

**When to use warmup**:
- With large learning rates (> 1e-3)
- With Set Transformer (attention mechanisms are sensitive to early optimisation)
### Cosine Annealing

After warmup, cosine annealing smoothly decays the learning rate following a half-cosine curve:

```
lr(t) = lr_min + 0.5 × (lr_max - lr_min) × (1 + cos(π × t / T))
```

The curve starts flat (high LR for a while), then accelerates the decay, then flattens again near zero:

```
lr ──┐
     │___
     │   ╲
     │    ╲
     │     ╲__
     └────────── epoch
```

This is better than linear decay because it spends more time at higher learning rates (making progress) and more time at lower learning rates (fine-tuning), with a smooth transition.

```python
config = mimic.TrainConfig(
    lr=3e-4,
    warmup_epochs=5,
    scheduler="cosine",
)
```

### How Warmup and Scheduling Compose

Mimic uses PyTorch's `SequentialLR` to chain them:

```
epoch:  [0 ... warmup_epochs ... total_epochs]
         |← linear warmup →|← cosine decay →|
```

The warmup runs first, then the cosine schedule takes over for the remaining epochs.

---

## Configuration Reference

```python
@dataclass
class TrainConfig:
    epochs: int = 50
    lr: float = 3e-4
    device: str = "cpu"
    warmup_epochs: int = 0
    scheduler: Literal["none", "cosine"] = "none"
```

### Recommended Configurations

**Quick experiment** (default):
```python
TrainConfig(epochs=50, lr=3e-4)
```

**Solid training run**:
```python
TrainConfig(epochs=200, lr=3e-4, warmup_epochs=10, scheduler="cosine")
```

---

## Epoch Callback

The trainer accepts an optional callback for monitoring:

```python
def on_epoch(epoch: int, total_epochs: int, avg_loss: float, current_lr: float):
    print(f"Epoch {epoch+1}/{total_epochs}: loss={avg_loss:.4f}, lr={current_lr:.1e}")

result = trainer.fit(dataloader, epoch_callback=on_epoch)
```

The CLI uses this to render the coloured loss bars you see in the terminal.

## TrainResult

After training, you get back a `TrainResult`:

```python
result.epoch_losses   # list[float] — loss per epoch
result.epoch_lrs      # list[float] — learning rate per epoch
result.final_loss     # float — last epoch's loss
```

Use `epoch_losses` to plot training curves or detect overfitting. Use `epoch_lrs` to verify your warmup/schedule is behaving as expected.
