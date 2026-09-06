"""Measure extraction quality against a hand-labelled gold set.

Scoring is **pure** — it compares a predictions frame to a gold frame — so it is
testable without a model. ``evaluate`` wires extraction and scoring together for
the model-running path.

A gold frame is just an input frame (``id`` + ``description`` [+ image columns])
with extra columns holding the *expected* value for each schema feature. Cells
left blank are unlabelled and excluded from a field's support, so a partial gold
set is fine: label only what you are sure of.
"""

from __future__ import annotations

from dataclasses import dataclass

import polars as pl

from extract.pipeline import extract_features
from extract.schema import ExtractionField, ExtractionSchema, FieldKind


@dataclass(frozen=True)
class FieldScore:
    field: str
    support: int  # gold rows carrying a non-null label for this field
    correct: int

    @property
    def accuracy(self) -> float | None:
        return self.correct / self.support if self.support else None


@dataclass(frozen=True)
class EvalReport:
    per_field: tuple[FieldScore, ...]

    @property
    def micro_accuracy(self) -> float | None:
        """Correct over every labelled cell — weights fields by their support."""
        support = sum(f.support for f in self.per_field)
        correct = sum(f.correct for f in self.per_field)
        return correct / support if support else None

    @property
    def macro_accuracy(self) -> float | None:
        """Mean of per-field accuracies — every field weighs the same."""
        accs = [f.accuracy for f in self.per_field if f.accuracy is not None]
        return sum(accs) / len(accs) if accs else None


def score_frame(
    pred: pl.DataFrame,
    gold: pl.DataFrame,
    schema: ExtractionSchema,
    *,
    key: str = "id",
) -> EvalReport:
    """Score predictions against gold, one :class:`FieldScore` per labelled field.

    A prediction is correct when it matches the gold value under the field's
    kind (numeric tolerance for numbers, case/spacing-insensitive otherwise). A
    null prediction against a labelled gold cell counts as a miss.
    """
    kinds = {f.name: f.kind for f in _flatten(schema)}
    pred_by_key = {row[key]: row for row in pred.iter_rows(named=True)}

    scores: list[FieldScore] = []
    for field in schema.column_names():
        if field not in gold.columns:
            continue
        kind = kinds.get(field, FieldKind.TEXT)
        support = correct = 0
        for grow in gold.iter_rows(named=True):
            gold_val = grow.get(field)
            if gold_val is None or gold_val == "":
                continue
            support += 1
            prediction = pred_by_key.get(grow[key], {}).get(field)
            if _match(kind, gold_val, prediction):
                correct += 1
        if support:
            scores.append(FieldScore(field, support, correct))
    return EvalReport(tuple(scores))


def evaluate(
    gold: pl.DataFrame,
    schema: ExtractionSchema,
    extractor,
    *,
    key: str = "id",
    description_col: str = "description",
    image_col: str = "image_path",
    image_url_col: str = "image_url",
) -> tuple[pl.DataFrame, EvalReport]:
    """Run extraction over the gold inputs and score the result.

    Returns ``(predictions, report)``. The gold's label columns are dropped
    before extraction so they cannot leak into the model inputs.
    """
    label_cols = {c for c in schema.column_names() if c in gold.columns}
    inputs = gold.select([c for c in gold.columns if c not in label_cols])
    pred = extract_features(
        inputs,
        schema,
        extractor,
        description_col=description_col,
        image_col=image_col,
        image_url_col=image_url_col,
    )
    return pred, score_frame(pred, gold, schema, key=key)


def _flatten(schema: ExtractionSchema) -> list[ExtractionField]:
    out: list[ExtractionField] = []
    for f in schema.fields:
        out.append(f)
        for subs in f.children.values():
            out.extend(subs)
    return out


def _norm(value: object) -> str:
    return str(value).strip().lower().replace(" ", "_")


def _match(kind: FieldKind, gold: object, pred: object) -> bool:
    if pred is None:
        return False
    if kind in (FieldKind.NUMBER, FieldKind.INTEGER):
        try:
            return abs(float(gold) - float(pred)) < 1e-9  # type: ignore[arg-type]
        except (TypeError, ValueError):
            return False
    if kind is FieldKind.BOOLEAN:
        return bool(gold) == bool(pred)
    return _norm(gold) == _norm(pred)  # text / categorical
