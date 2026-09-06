"""Schema is the single source of truth: prompt out, validation in. No model."""

from __future__ import annotations

import json

import pytest

from extract.domains import get_schema
from extract.schema import ExtractionField as F
from extract.schema import ExtractionSchema, FieldKind, load_schema


@pytest.fixture
def clothing_schema():
    return get_schema("clothing")


def test_categorical_field_requires_choices():
    with pytest.raises(ValueError, match="must define choices"):
        F("category", "type", FieldKind.CATEGORICAL)


def test_children_require_categorical_parent():
    with pytest.raises(ValueError, match="not categorical"):
        F("brand", "brand name", FieldKind.TEXT, children={"x": (F("y", "y"),)})


def test_column_names_lists_parents_before_children_without_duplicates(clothing_schema):
    cols = clothing_schema.column_names()
    assert cols.index("category") < cols.index("dress_type")
    assert cols.index("category") < cols.index("footwear_type")
    assert len(cols) == len(set(cols))


def test_prompt_block_describes_fields_and_conditionals(clothing_schema):
    prompt = clothing_schema.prompt_block()
    assert "clothing" in prompt
    assert "dress" in prompt and "footwear" in prompt
    assert 'If "category" == "dress"' in prompt
    assert "dress_type" in prompt


def test_validate_normalises_categorical_to_known_choice(clothing_schema):
    out = clothing_schema.validate({"category": "Dress", "primary_colour": "Red"})
    assert out["category"] == "dress"
    assert out["primary_colour"] == "Red"


def test_validate_keeps_off_list_value_but_normalised(clothing_schema):
    # The model may be more specific than our taxonomy: keep the answer
    # (snake_cased) rather than drop it — but an off-list parent unlocks nothing.
    out = clothing_schema.validate({"category": "Maxi Dress"})
    assert out["category"] == "maxi_dress"
    assert "dress_type" not in out


def test_validate_unlocks_children_only_when_parent_fires(clothing_schema):
    is_dress = clothing_schema.validate({"category": "dress", "dress_type": "Maxi"})
    assert is_dress["dress_type"] == "maxi"

    is_top = clothing_schema.validate({"category": "top", "dress_type": "maxi"})
    assert "dress_type" not in is_top


def test_validate_empties_become_null(clothing_schema):
    out = clothing_schema.validate({"category": "", "material": ""})
    assert out["category"] is None
    assert out["material"] is None


def test_validate_coerces_boolean_and_number():
    schema = ExtractionSchema(
        "demo",
        "demo domain",
        fields=(
            F("in_stock", "availability", FieldKind.BOOLEAN),
            F("price", "price", FieldKind.NUMBER),
        ),
    )
    out = schema.validate({"in_stock": "yes", "price": "19.99"})
    assert out["in_stock"] is True
    assert out["price"] == pytest.approx(19.99)

    out = schema.validate({"in_stock": False, "price": "n/a"})
    assert out["in_stock"] is False
    assert out["price"] is None


def test_number_tolerates_units():
    schema = ExtractionSchema(
        "demo", "demo", fields=(F("weight", "weight", FieldKind.NUMBER),)
    )
    # "2.5 kg" would have been dropped to null by a bare float() call.
    assert schema.validate({"weight": "2.5 kg"})["weight"] == pytest.approx(2.5)


def test_integer_kind_parses_to_int_with_units():
    schema = ExtractionSchema(
        "demo",
        "demo",
        fields=(
            F("vram", "VRAM in GB", FieldKind.INTEGER),
            F("cores", "core count", FieldKind.INTEGER),
        ),
    )
    out = schema.validate({"vram": "16 GB", "cores": 8})
    assert out["vram"] == 16
    assert isinstance(out["vram"], int)
    assert out["cores"] == 8
    assert schema.validate({"vram": "n/a"})["vram"] is None


def test_json_schema_maps_kinds_and_makes_categoricals_nullable_enums(clothing_schema):
    js = clothing_schema.json_schema()
    assert js["type"] == "object"
    props = js["properties"]
    # Categorical → nullable enum carrying the taxonomy plus null.
    assert "dress" in props["category"]["enum"]
    assert None in props["category"]["enum"]
    # Conditional children are flattened in as optional properties.
    assert "dress_type" in props
    # Integer kind surfaces as an integer type.
    elec = get_schema("electronics").json_schema()["properties"]
    assert "integer" in elec["vram_gb"]["type"]


def test_few_shot_examples_render_into_the_prompt():
    schema = ExtractionSchema(
        "demo",
        "demo",
        fields=(F("category", "c", FieldKind.CATEGORICAL, ("dress", "top")),),
        examples=({"input": "a red dress", "output": {"category": "dress"}},),
    )
    prompt = schema.prompt_block()
    assert "Example:" in prompt
    assert "a red dress" in prompt
    assert '"category": "dress"' in prompt


def test_declarative_round_trip_through_json(tmp_path):
    data = {
        "domain": "furniture",
        "description": "home furniture",
        "fields": [
            {
                "name": "category",
                "kind": "categorical",
                "description": "type",
                "choices": ["chair", "table"],
                "children": {
                    "chair": [
                        {
                            "name": "chair_style",
                            "kind": "categorical",
                            "choices": ["dining", "office"],
                        }
                    ]
                },
            },
            {"name": "primary_material", "description": "main material"},
        ],
    }
    path = tmp_path / "furniture.json"
    path.write_text(json.dumps(data))
    schema = load_schema(path)
    assert schema.domain == "furniture"
    assert schema.column_names() == ["category", "chair_style", "primary_material"]
    # The loaded schema behaves like a hand-written one.
    out = schema.validate({"category": "chair", "chair_style": "Office"})
    assert out["chair_style"] == "office"
