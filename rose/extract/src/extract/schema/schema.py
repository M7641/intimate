"""Data-driven extraction schema.

The pipeline is domain-agnostic: a domain (clothing, electronics, ...) is just
an instance of :class:`ExtractionSchema`. The schema knows how to render the
instructions sent to the VLM, validate/normalise the JSON output, and enumerate
the feature columns (including conditional fields).
"""

from __future__ import annotations

import json
import re
from dataclasses import dataclass, field
from enum import Enum
from pathlib import Path


class FieldKind(str, Enum):
    CATEGORICAL = "categorical"  # one value out of a fixed set
    TEXT = "text"  # short free-form text
    BOOLEAN = "boolean"
    NUMBER = "number"  # any real number
    INTEGER = "integer"  # whole number (counts, sizes in GB, ...)


@dataclass(frozen=True)
class ExtractionField:
    """A single feature to extract.

    Conditional fields model the hierarchy: ``children`` maps a parent
    categorical value to the sub-fields it unlocks. For example,
    ``category == "dress"`` unlocks a ``dress_type`` field.
    """

    name: str
    description: str
    kind: FieldKind = FieldKind.TEXT
    choices: tuple[str, ...] = ()
    children: dict[str, tuple[ExtractionField, ...]] = field(default_factory=dict)

    def __post_init__(self) -> None:
        if self.kind is FieldKind.CATEGORICAL and not self.choices:
            msg = f"Categorical field {self.name!r} must define choices"
            raise ValueError(msg)
        if self.children and self.kind is not FieldKind.CATEGORICAL:
            msg = f"Field {self.name!r} has children but is not categorical"
            raise ValueError(msg)


@dataclass(frozen=True)
class ExtractionSchema:
    domain: str
    description: str
    fields: tuple[ExtractionField, ...]
    # Optional few-shot exemplars: {"input": "<description>", "output": {...}}.
    examples: tuple[dict, ...] = ()

    @classmethod
    def from_dict(cls, data: dict) -> ExtractionSchema:
        """Build a schema from a plain dict (loaded from JSON/YAML)."""
        return cls(
            domain=data["domain"],
            description=data["description"],
            fields=tuple(_field_from_dict(f) for f in data["fields"]),
            examples=tuple(data.get("examples", ())),
        )

    def column_names(self) -> list[str]:
        """All possible feature columns, parents first then children."""
        names: list[str] = []
        for f in self.fields:
            names.append(f.name)
            for sub in f.children.values():
                for child in sub:
                    if child.name not in names:
                        names.append(child.name)
        return names

    def prompt_block(self) -> str:
        """Render the instruction block describing the output contract."""
        lines = [
            f"Product domain: {self.domain} — {self.description}",
            "",
            "Fill in the following attributes. If a piece of information is",
            "missing, use the value null. Reply with ONE JSON object only.",
            "",
            "Attributes:",
        ]
        for f in self.fields:
            lines.append(f"  - {_describe_field(f)}")
        conditional = [
            (f, value, subs) for f in self.fields for value, subs in f.children.items()
        ]
        if conditional:
            lines += ["", "Conditional attributes:"]
            for f, value, subs in conditional:
                for child in subs:
                    lines.append(
                        f'  - If "{f.name}" == "{value}", also add '
                        + _describe_field(child)
                    )
        for ex in self.examples:
            lines += [
                "",
                "Example:",
                f"  Description: {ex.get('input', '')}",
                f"  Output: {json.dumps(ex.get('output', {}), ensure_ascii=False)}",
            ]
        return "\n".join(lines)

    def json_schema(self) -> dict:
        """A JSON Schema for the output object — one property per feature.

        Categorical fields become nullable ``enum``s; numbers/integers/booleans
        get their type. Conditional children appear as flat optional properties
        (JSON Schema if/then would over-complicate the grammar). This is the
        contract a constrained-decoding backend enforces.
        """
        properties: dict[str, dict] = {}
        for f in self.fields:
            properties[f.name] = _field_json_schema(f)
            for subs in f.children.values():
                for child in subs:
                    properties.setdefault(child.name, _field_json_schema(child))
        return {
            "type": "object",
            "properties": properties,
            "additionalProperties": False,
        }

    def validate(self, raw: dict) -> dict:
        """Normalise the model's raw output into the expected columns."""
        out: dict[str, object] = {}
        for f in self.fields:
            value = _coerce(f, raw.get(f.name))
            out[f.name] = value
            # Unlock sub-fields when the parent value matches.
            subs = f.children.get(value) if isinstance(value, str) else None
            for child in subs or ():
                out[child.name] = _coerce(child, raw.get(child.name))
        return out


def load_schema(path: str | Path) -> ExtractionSchema:
    """Load a schema from a declarative ``.json`` / ``.yaml`` file.

    Lets non-developers add a domain without touching Python. YAML requires the
    optional ``pyyaml`` dependency; JSON works out of the box.
    """
    path = Path(path)
    text = path.read_text(encoding="utf-8")
    if path.suffix in (".yaml", ".yml"):
        import yaml  # optional; only needed for YAML schemas

        data = yaml.safe_load(text)
    else:
        data = json.loads(text)
    return ExtractionSchema.from_dict(data)


def _field_from_dict(d: dict) -> ExtractionField:
    children = {
        value: tuple(_field_from_dict(c) for c in subs)
        for value, subs in (d.get("children") or {}).items()
    }
    return ExtractionField(
        name=d["name"],
        description=d.get("description", ""),
        kind=FieldKind(d.get("kind", "text")),
        choices=tuple(d.get("choices", ())),
        children=children,
    )


_JSON_TYPE = {
    FieldKind.TEXT: "string",
    FieldKind.CATEGORICAL: "string",
    FieldKind.BOOLEAN: "boolean",
    FieldKind.NUMBER: "number",
    FieldKind.INTEGER: "integer",
}


def _field_json_schema(f: ExtractionField) -> dict:
    spec: dict = {"type": [_JSON_TYPE[f.kind], "null"]}
    if f.kind is FieldKind.CATEGORICAL:
        spec["enum"] = [*f.choices, None]
    return spec


def _describe_field(f: ExtractionField) -> str:
    head = f'"{f.name}"'
    if f.kind is FieldKind.CATEGORICAL:
        head += f" (one of: {', '.join(f.choices)})"
    elif f.kind is FieldKind.BOOLEAN:
        head += " (true or false)"
    elif f.kind is FieldKind.NUMBER:
        head += " (number)"
    elif f.kind is FieldKind.INTEGER:
        head += " (integer)"
    return f"{head}: {f.description}"


_NUMERIC = re.compile(r"[-+]?\d*\.?\d+")


def _parse_number(value: object) -> float | None:
    """First numeric token in ``value`` — tolerates units like '16 GB', '8cm'."""
    if isinstance(value, bool):  # bool is an int subclass; don't treat as number
        return None
    if isinstance(value, (int, float)):
        return float(value)
    match = _NUMERIC.search(str(value).replace(",", ""))
    return float(match.group()) if match else None


def _coerce(f: ExtractionField, value: object) -> object:
    if value is None or value == "":
        return None
    if f.kind is FieldKind.CATEGORICAL and isinstance(value, str):
        norm = value.strip().lower().replace(" ", "_")
        # Keep the normalised raw value even if off-list: the model may be more
        # specific than our choices (worth surfacing in downstream validation).
        for choice in f.choices:
            if norm == choice.lower():
                return choice
        return norm
    if f.kind is FieldKind.BOOLEAN:
        if isinstance(value, bool):
            return value
        return str(value).strip().lower() in {"true", "yes", "1"}
    if f.kind is FieldKind.NUMBER:
        return _parse_number(value)
    if f.kind is FieldKind.INTEGER:
        n = _parse_number(value)
        return int(round(n)) if n is not None else None
    return str(value).strip()
