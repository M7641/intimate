"""Concrete domain schemas.

These are *data*, not code: adding a domain happens here without touching the
pipeline or the extractor.
"""

from __future__ import annotations

from extract.schema import ExtractionField as F
from extract.schema import ExtractionSchema, FieldKind

CLOTHING = ExtractionSchema(
    domain="clothing / apparel",
    description="garments and fashion accessories",
    fields=(
        F(
            name="category",
            description="top-level garment type",
            kind=FieldKind.CATEGORICAL,
            choices=(
                "dress",
                "top",
                "skirt",
                "trousers",
                "outerwear",
                "knitwear",
                "footwear",
                "accessory",
            ),
            children={
                "dress": (
                    F(
                        name="dress_type",
                        description="dress sub-type",
                        kind=FieldKind.CATEGORICAL,
                        choices=(
                            "maxi",
                            "midi",
                            "mini",
                            "wrap",
                            "shirt_dress",
                            "bodycon",
                            "a_line",
                            "slip",
                        ),
                    ),
                ),
                "footwear": (
                    F(
                        name="footwear_type",
                        description="footwear sub-type",
                        kind=FieldKind.CATEGORICAL,
                        choices=("heels", "boots", "trainers", "flats", "sandals"),
                    ),
                ),
            },
        ),
        F(name="primary_colour", description="dominant colour of the item"),
        F(name="material", description="main fabric or material"),
        F(name="pattern", description="pattern (solid, striped, floral, ...)"),
        F(
            name="sleeve_length",
            description="sleeve length if relevant",
            kind=FieldKind.CATEGORICAL,
            choices=("sleeveless", "short", "three_quarter", "long"),
        ),
        F(name="brand", description="brand if visible or mentioned"),
    ),
    examples=(
        {
            "input": "Black bodycon mini dress in ribbed jersey, sleeveless, by Zara",
            "output": {
                "category": "dress",
                "dress_type": "bodycon",
                "primary_colour": "black",
                "material": "jersey",
                "pattern": "solid",
                "sleeve_length": "sleeveless",
                "brand": "zara",
            },
        },
    ),
)

ELECTRONICS = ExtractionSchema(
    domain="electronics / components",
    description="computer hardware and components",
    fields=(
        F(
            name="category",
            description="top-level component type",
            kind=FieldKind.CATEGORICAL,
            choices=(
                "motherboard",
                "gpu",
                "cpu",
                "ram",
                "storage",
                "psu",
                "peripheral",
            ),
            children={
                "motherboard": (
                    F(name="chipset", description="chipset (e.g. Z790, B650, X670E)"),
                    F(
                        name="form_factor",
                        description="board form factor",
                        kind=FieldKind.CATEGORICAL,
                        choices=("atx", "micro_atx", "mini_itx", "e_atx"),
                    ),
                    F(name="socket", description="CPU socket (e.g. LGA1700, AM5)"),
                ),
                "gpu": (
                    F(name="gpu_chip", description="graphics chip (e.g. RTX 4070)"),
                    F(name="vram_gb", description="VRAM in GB", kind=FieldKind.INTEGER),
                ),
                "cpu": (
                    F(name="cpu_family", description="family (e.g. Ryzen 7, Core i5)"),
                    F(
                        name="core_count",
                        description="number of cores",
                        kind=FieldKind.INTEGER,
                    ),
                ),
            },
        ),
        F(name="brand", description="brand / manufacturer"),
        F(name="model", description="exact part number or model"),
    ),
    examples=(
        {
            "input": "ASUS ROG STRIX B650-E Gaming WiFi, ATX, AM5 socket",
            "output": {
                "category": "motherboard",
                "chipset": "B650E",
                "form_factor": "atx",
                "socket": "AM5",
                "brand": "asus",
                "model": "ROG STRIX B650-E GAMING WIFI",
            },
        },
    ),
)

SCHEMAS: dict[str, ExtractionSchema] = {
    "clothing": CLOTHING,
    "electronics": ELECTRONICS,
}


def get_schema(name: str) -> ExtractionSchema:
    try:
        return SCHEMAS[name]
    except KeyError:
        available = ", ".join(sorted(SCHEMAS))
        msg = f"Unknown domain {name!r}. Available: {available}"
        raise KeyError(msg) from None
