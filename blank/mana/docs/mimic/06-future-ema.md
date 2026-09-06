# Future Consideration: EMA (Exponential Moving Average) Encoder

## What It Is

A momentum-based encoder that maintains two copies of the network:

1. **Main encoder**: trained normally via backprop
2. **Momentum encoder**: a slow-moving average updated after each step:
   ```
   θ_momentum = m × θ_momentum + (1 - m) × θ_main
   ```
   where `m` is typically 0.996–0.999.

The momentum encoder provides **stable targets** that evolve slowly, reducing the noise from rapidly changing weights during training.

## Why We Removed It

We previously had an `EMAEncoder` wrapper, but it was never properly integrated:

- `MimicModel.forward()` encoded **both views** through the main encoder
- `forward_momentum()` was never called during training
- The momentum encoder doubled parameter count and added per-step overhead with zero impact on loss

To be useful, the model would need **asymmetric encoding**: one view through the main encoder, the other through the momentum encoder. This is a non-trivial architectural change.

## When to Revisit

EMA is the backbone of several important methods:

- **BYOL** (Bootstrap Your Own Latent): EMA + predictor head + stop-gradient, no negatives needed
- **MoCo** (Momentum Contrast): EMA encoder maintains a queue of consistent negatives
- **DINO**: EMA teacher for self-distillation

Consider adding it back if:

- Training becomes unstable with symmetric encoding at larger scale
- We want to support negative-free methods (BYOL) — useful when batch sizes are too small for good negatives
- We need a momentum queue for very large negative pools (MoCo)

## Implementation Notes for When We Do

1. `MimicModel` needs an asymmetric forward path: view_1 → main encoder, view_2 → momentum encoder (with stop-gradient)
2. Momentum schedule should ramp from `m_base → 1.0` via cosine: `m(t) = 1 - (1-m_base)(1+cos(πt))/2`
3. BYOL additionally needs a **predictor head** (small MLP) on the main encoder's output only
4. Momentum encoder parameters must have `requires_grad=False`
5. Call `ema.update()` after each `optimizer.step()`, not after each epoch
