# Directions — where this could go next

This document maps the design space _beyond_ what the module builds today. For
what already exists and the theory behind it, see [`design.md`](design.md); for
how to run it, see the [README](../README.md).

The framing that drives all of this (from `design.md`): **the cost is decoding,
not encoding**, and **the LLM does not have to run per product** — it can run
per cluster, per sample, or not at all. The directions below are instances of
pushing that further, grouped by how much work they take and how high they
reach.

## Group A — reuse embeddings, no generation (partly built)

The module already ships embeddings-as-features, embed→cluster→name, zero-shot
retrieval, and span extraction (see `design.md`). Two pieces of wiring would
turn the clustering path from a per-file experiment into a stable incremental
feature:

- **Persist cluster centroids in the warehouse**, the way the PCA projection is
  already persisted. `assign_to_centroids` is the incremental path; it needs
  stored centroids to assign new products against. Without this, every run
  re-clusters from scratch and cluster ids drift.
- **An ANN index over centroids.** The greedy clustering paths (`condense`,
  `cluster`, near-dedup) compare each new vector against every centroid
  (`O(new · clusters)`). At scale, an approximate-nearest-neighbour index makes
  assignment sub-linear.

## Group B — learn a focused encoder

Groups A use _general_ embeddings. Tuning the representation to the retail task
costs more work but raises the ceiling.

### B1. Set Transformer for focused embeddings

Frame a product as a **set** — a bag of attribute strings, n-grams, or the
several description fields (`product_name`, `heading_text`, `product_copy`) we
already pull in `datasource`. A Set Transformer (Lee et al., 2019; ISAB encoder +
PMA pooling) produces a single permutation-invariant embedding over that set.

- **When it pays:** when the input is genuinely _unordered_ — multiple fields,
  scraped attribute lists, bullet points — rather than one ordered prose blob. A
  standard sequence encoder treats prose better; a set encoder treats sets
  better.
- **The real lever is the training objective**, not the architecture. A set
  transformer with no task signal just gives another general embedding. To make
  it _focused_ it needs a loss: contrastive (pull same-product-type descriptions
  together), or supervised against whatever labels we trust. Which leads to:

### B2. Distillation — slow LLM teacher → fast student

Run the **current slow LLM once, on a sample** (say 50–100k products) to produce
labels. Train a fast student on `(embedding → label)`: a logistic-regression head,
a small fine-tuned encoder (DistilBERT), or the set transformer from B1. Then run
the _cheap student_ over the full catalogue — a forward pass per product.

You pay the LLM cost once, on a bounded sample, and amortise it across millions of
cheap inferences. This is the standard "label with the expensive model, serve with
the cheap model" pattern, and it directly turns Stage 1 into a **one-off labelling
job** instead of a recurring per-product cost.

Two refinements that change the economics:

- **Pick the sample actively, not randomly.** A random 50–100k over-pays for
  easy cases. Select by uncertainty (low `extract_consistent` agreement) or by
  diversity (cover the embedding clusters) — typically 2–5× less labelling for
  the same student quality.
- **Audit the teacher's labels before training.** Distilling errors bakes them
  in. Cross the teacher's output against the consistency signal, or run a
  label-noise pass (cleanlab-style) over the sample, and re-label or drop the
  suspicious slice.

### B3. SetFit — contrastive few-shot

SetFit fine-tunes a sentence-transformer with contrastive pairs from a _tiny_
labelled set (8–32 examples per class) plus a light classifier head. Extremely
sample-efficient, no prompt, fast inference.

Pairs naturally with B2: use the LLM (or a human) to seed a handful of examples
per class, let SetFit scale them to the catalogue. A strong middle ground when we
have classes in mind but little labelled data.

## Group C — keep generation, make it cheaper

If we want to retain the LLM's open-vocabulary flexibility, attack the _speed_
rather than replace the approach. Lower risk, smaller ceiling.

### A faster serving backend

The pipeline runs transformers' `generate()`. A serving engine like vLLM (paged
attention + continuous batching) commonly reaches 5–20× the throughput on a GPU.
This was prototyped behind a `--backend vllm` switch and then **removed**: it
never earned its keep on our hardware, and vLLM is Linux/CUDA-only and pins an
older `torchvision` than the core dep, so it could not live in the lockfile and
added a backend abstraction across the CLI, the extractor and the deploy spec for
a single extra engine. Keeping the module installable and one-engine-simple won
out.

If a faster serving path is needed later, it can return behind the same
`TextExtractor` public surface (`build_messages` + `schema.validate`) so it stays
comparable on the same gold set — and it belongs in the GPU image, installed into
its own environment, not co-resolved with the CPU core.

### Smaller model + constrained decoding

Constrained decoding (already exposed as `--constrained`) both guarantees valid
JSON _and_ prunes the token space, which speeds decoding. The open experiment:
pair `--constrained` with the **smallest** model that still passes the eval — the
0.5B may suffice once output is constrained to a short label.

## Group D — hybrid / routing

The options above are not exclusive. The best system is likely a cascade.

### D1. Confidence-routed cascade

Most products are easy: "Nike running shoe" → `trainers` is obvious to the
cluster / retrieval paths. Send the easy, high-confidence majority down the cheap
path, and **route only the low-confidence residual to the slow LLM**. The
confidence signals already exist — `extract_consistent` (per-field agreement) and
the cosine `margin` emitted by `extract classify`.

If 90% of the catalogue is easy, this cuts the LLM bill 10× while keeping its
quality exactly where it is needed: the ambiguous tail. It composes a cheap
Group-A/B path with a Group-C slow path, chaining the `*_frame` surfaces the
modules already expose.

One missing piece: a raw confidence signal is not a threshold. Plot the
coverage/accuracy curve on the gold set and pick the operating point
deliberately — otherwise "90% of the catalogue is easy" is a hope, not a
measurement.

### D2. Rules / weak-supervision baseline

Before anything fancy: TF-IDF + linear model, a gazetteer of product nouns, or
**head-noun extraction** (the syntactic head of the noun phrase is often
_literally_ the product type — spaCy gives it for free). Cheap, interpretable,
and a strong baseline that tells us how much the LLM is actually buying us. Also
a useful _feature_ in its own right inside D1.

## A suggested order of experiments

Cheapest-first, so each step de-risks the next:

0. **Step zero:** dedup + cache; measure the volume reduction. Multiplies
   everything after it. (The mechanism ships; the open part is measuring the
   actual duplicate rate on the warehouse data — that number decides how much
   the other levers matter.)
1. **Baseline:** D2 head-noun + retrieval against the current concept names.
   Establishes how far free/instant methods already get us.
2. **Spans:** GLiNER + regex on the non-category fields. The only cheap path that
   covers the rest of the schema.
3. **Cluster:** cluster raw descriptions, name clusters once. The likely big win.
4. **Distillation (B2):** if the cheap paths leave accuracy on the table, distil
   the LLM into a student.
5. **Routed cascade (D1):** wrap whichever cheap path won with an LLM fallback on
   the hard tail, thresholded on the gold-set coverage/accuracy curve.

## The design space at a glance

| Option                    | Output              | Generation?     | Rel. cost   | Effort  |
| ------------------------- | ------------------- | --------------- | ----------- | ------- |
| Step-0 dedup + cache      | volume cut          | none            | ~free       | trivial |
| Embeddings as features    | features (b)        | none            | very low    | trivial |
| Cluster → name once       | label (a)           | per-cluster     | low         | low     |
| Zero-shot retrieval       | label (a)           | none            | very low    | low     |
| Span extraction           | all schema fields   | none            | very low    | low     |
| Set transformer (B1)      | features (b)        | none            | low (infer) | high    |
| Distil LLM → student (B2) | label (a)           | once, on sample | low (infer) | medium  |
| SetFit few-shot (B3)      | label (a)           | none            | low         | medium  |
| Faster serving backend    | label (a)           | per-product     | medium      | low     |
| Small + constrained       | label (a)           | per-product     | medium      | low     |
| Prompt batching           | label (a)           | per-batch       | medium      | trivial |
| Routed cascade (D1)       | label (a)           | hard tail only  | tunable     | medium  |
| Rules / head-noun (D2)    | label (a) + feature | none            | very low    | low     |

## The bet

If forced to choose: **cluster + routed cascade.** Cluster the raw descriptions
directly (killing the per-product generation), name each cluster once with the
LLM, and keep an LLM fallback for the low-confidence tail. It reuses the most
existing code, removes the bottleneck rather than merely shrinking it, and
degrades gracefully. Distillation is the natural follow-up if we need higher
accuracy at full catalogue scale.

Two riders: run step-0 dedup first regardless of the winner, and pair the cluster
category with span extraction so the schema's _non-category_ fields are covered
without generation too.
