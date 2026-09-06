# augury — a time series embedding you tune onto other problems

The spark: [TimesFM](https://github.com/google-research/timesfm) and its siblings
(Chronos, MOMENT, Moirai) are **foundation models for time series** — one heavy
encoder, pre-trained on a massive corpus of series, that turns a window of
numbers into a vector. They follow the same recipe as BERT and CLIP: a frozen
encoder, then **light heads** bolted on per task.

This pilot tests one hypothesis, cleanly:

> A single, **frozen** time series embedding carries enough signal that a tiny
> head (a ridge regression, a logistic classifier, a kNN) solves several
> downstream problems — without re-training the encoder, without feature
> engineering per task.

---

## Part 0 — Separate the two ideas

It is easy to conflate two things that live one layer apart. Keep them apart:

1. **"Make a forecast."** Chronos / TimesFM already do this **zero-shot**. No
   training, no head — it is an API call, not a research question.
2. **"An embedding you tune onto other problems."** _This_ is the hypothesis:
   that one gelled vector is reusable across tasks, each task adding only a
   linear head on top.

So the forecast is not the prize. It is the **first falsifiable test** of idea
n°2 — the one task where we have a strong baseline and hard numbers to argue
with.

---

## Part 1 — The recipe

```
  raw series ──► patcher ──► [ pre-trained encoder, FROZEN ] ──► z (embedding) ──► head ──► output
                                                                      │
                          ┌────────────────────────────────────────────┼────────────────────────────┐
                      forecast head                          classifier head                   kNN / similarity
                      (ridge / MLP)                          (SKU? promo period?)              "find me series that
                      ← THE PROOF                            ← THE TRANSFER                      behave like this one"
```

The whole pilot rests on **one primitive**: `embed(series) -> vector`. Exactly
as `saga` rests on `embed`. Everything else is a head. That is what makes the
demo legible — **one embedding, many heads** — and what makes the claim
testable: if the embedding is hollow, every head fails together.

Two granularities of embedding, both needed:

- **per-window** — one vector per sliding context window → feeds the forecast
  head, which needs to map "the recent shape" to "what comes next";
- **per-series (pooled)** — mean over windows → a fixed summary of a whole
  series → feeds classification and retrieval.

---

## Part 2 — Why the forecast is the right first proof

We freeze the encoder and ask it the sharpest question we can score:

> Does a ridge regression on the frozen embedding forecast **as well as**
> (a) the foundation model's own native forecast, and (b) a classical baseline?

The three-way comparison is the experiment:

| Forecaster                     | What it tells us                                                            |
| ------------------------------ | --------------------------------------------------------------------------- |
| **seasonal-naive**             | the floor — if we can't beat last season, stop                              |
| **Chronos native (zero-shot)** | what the foundation model gives for free                                    |
| **ridge on frozen embedding**  | does the _embedding_ retain the predictive signal, or only the native head? |

- Embedding-head ≈ native → the vector carries the signal. We have earned the
  right to reuse it for the transfer tasks.
- Embedding-head ≪ native → the signal lives in the decoder, not the vector;
  for our data the embedding needs fine-tuning, not just a probe.

Either outcome is a result. That is the point of leading with forecast.

---

## Part 3 — Which model to wrap (do not pre-train our own)

For a pilot we **wrap an existing checkpoint**. Pre-training a time series
encoder costs GPUs and will not beat a 200M-param model zero-shot. The menu:

| Model                     | Backbone              | Strong at                   | Note                                                               |
| ------------------------- | --------------------- | --------------------------- | ------------------------------------------------------------------ |
| **Chronos-Bolt** (Amazon) | T5, tokenized values  | zero-shot forecast          | the "just works" forecaster — **our anchor**                       |
| **MOMENT** (CMU)          | masked T5             | **multi-task embeddings**   | embedding is first-class; the natural model for the transfer tasks |
| TimesFM (Google)          | decoder-only, patched | zero-shot forecast          | the spark; univariate; heavier to install                          |
| Moirai (Salesforce)       | encoder               | multivariate, any-frequency | if/when covariates matter                                          |
| Lag-Llama                 | decoder-only, small   | easy fine-tuning            | if we move past frozen probing                                     |

We anchor on **Chronos** because the first milestone is forecast and Chronos is
the simplest strong zero-shot forecaster to run. **MOMENT** comes in at the
transfer milestone, where an embedding-native model earns its keep.

Both are PyTorch checkpoints on HuggingFace → this pilot is **Python + uv**
(like `saga`), not Rust. Running a transformer in Rust (candle) is possible but
a yak-shave that adds nothing to the research question.

---

## Part 4 — Shape of the pilot

```
augury/
  gensis.md          # this document
  README.md          # what / how to run
  pyproject.toml     # uv; deps: chronos-forecasting, momentfm, scikit-learn, polars
  augury/
    embed.py         # THE primitive: wrap Chronos/MOMENT -> vector (per-window + pooled)
    heads.py         # ridge forecast / logistic classifier / kNN
    backtest.py      # rolling-origin; MASE / sMAPE / pinball loss
    data.py          # load a few series (public benchmark first, warehouse later)
  moon.yml
```

### Milestone 1 — the proof

A script that: loads a handful of series → `embed()` (frozen) → trains a **ridge
regression** forecast head → backtests rolling-origin against **seasonal-naive**
and **Chronos native** → prints a metrics table (MASE, sMAPE, pinball). One
verdict: is the embedding-head in the same stadium as native?

### Milestone 2 — the payoff

The **same frozen embeddings** → a second task (classify series, or kNN
retrieval). Zero re-encoding, just a new head. This is where the
foundation-model dividend becomes tangible.

---

## Part 5 — Tensions to hold (not to solve in the pilot)

- **Frozen first, fine-tune later.** The frozen linear probe is the cleanest
  test of "is there transferable signal". Fine-tune (LoRA-style adapter on our
  data) only after the probe gives a verdict.
- **Covariates.** Chronos / TimesFM / MOMENT are **univariate** — the embedding
  ignores price, promo, holidays. For retail demand that is a blind spot, and it
  connects straight to `paradox` (elasticity, where price _is_ the subject).
  Flag it; don't fix it here.
- **Distribution shift.** Our series (intermittent demand, retail spikes) look
  little like the pre-training corpus → zero-shot may underperform. That gap is
  exactly what an embedding + light tune is meant to close.
- **De-risk on public data.** Start on a public benchmark (a Monash / M4 /
  electricity subset) before wiring the warehouse — it separates "the model
  works" from "our data is dirty".

---

## Part 6 — When NOT to reach for this

- You only need a forecast, on clean univariate series → call Chronos zero-shot
  and skip the embedding framing entirely.
- The drivers are exogenous (price, promo) and that is the whole question → a
  univariate embedding throws away the signal; go to `paradox` (causal /
  covariate-aware) instead.
- You have one series and lots of history → a local classical model (ETS,
  seasonal ARIMA) may beat a global foundation model with less machinery.
