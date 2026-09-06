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

FURNITURE = ExtractionSchema(
    domain="furniture / homeware",
    description="home and garden furniture and large homeware",
    fields=(
        F(
            name="category",
            description="top-level furniture type",
            kind=FieldKind.CATEGORICAL,
            choices=(
                "sofa",
                "chair",
                "table",
                "bed",
                "mattress",
                "storage",
                "lighting",
                "rug",
                "mirror",
                "outdoor",
            ),
            children={
                "sofa": (
                    F(
                        name="sofa_type",
                        description="sofa sub-type",
                        kind=FieldKind.CATEGORICAL,
                        choices=(
                            "corner",
                            "two_seater",
                            "three_seater",
                            "sofa_bed",
                            "recliner",
                            "modular",
                            "love_seat",
                        ),
                    ),
                    F(
                        name="seat_count",
                        description="number of seats",
                        kind=FieldKind.INTEGER,
                    ),
                ),
                "bed": (
                    F(
                        name="bed_size",
                        description="standard UK bed size",
                        kind=FieldKind.CATEGORICAL,
                        choices=(
                            "single",
                            "small_double",
                            "double",
                            "king",
                            "super_king",
                        ),
                    ),
                    F(
                        name="bed_type",
                        description="bed frame sub-type",
                        kind=FieldKind.CATEGORICAL,
                        choices=("divan", "frame", "ottoman", "bunk", "day_bed"),
                    ),
                ),
                "table": (
                    F(
                        name="table_type",
                        description="table sub-type",
                        kind=FieldKind.CATEGORICAL,
                        choices=(
                            "dining",
                            "coffee",
                            "side",
                            "console",
                            "nest",
                            "desk",
                        ),
                    ),
                ),
                "chair": (
                    F(
                        name="chair_type",
                        description="chair sub-type",
                        kind=FieldKind.CATEGORICAL,
                        choices=(
                            "dining",
                            "office",
                            "accent",
                            "bar_stool",
                            "stool",
                            "recliner",
                        ),
                    ),
                ),
                "storage": (
                    F(
                        name="storage_type",
                        description="storage sub-type",
                        kind=FieldKind.CATEGORICAL,
                        choices=(
                            "wardrobe",
                            "chest_of_drawers",
                            "bookcase",
                            "sideboard",
                            "cabinet",
                            "shelving",
                            "tv_unit",
                        ),
                    ),
                ),
            },
        ),
        F(name="primary_material", description="main frame / body material"),
        F(name="upholstery", description="covering fabric or leather if upholstered"),
        F(name="colour", description="dominant colour or finish"),
        F(name="style", description="design style (scandinavian, industrial, ...)"),
        F(
            name="room",
            description="intended room",
            kind=FieldKind.CATEGORICAL,
            choices=(
                "living_room",
                "bedroom",
                "dining_room",
                "office",
                "kitchen",
                "bathroom",
                "hallway",
                "nursery",
                "outdoor",
            ),
        ),
        F(
            name="assembly_required",
            description="whether self-assembly is needed",
            kind=FieldKind.BOOLEAN,
        ),
        F(name="brand", description="brand if visible or mentioned"),
    ),
    examples=(
        {
            "input": (
                "Grey fabric 3 seater corner sofa with chaise, foam-filled "
                "cushions, scandinavian style — flat-packed, self assembly"
            ),
            "output": {
                "category": "sofa",
                "sofa_type": "corner",
                "seat_count": 3,
                "primary_material": "wood",
                "upholstery": "fabric",
                "colour": "grey",
                "style": "scandinavian",
                "room": "living_room",
                "assembly_required": True,
                "brand": None,
            },
        },
        {
            "input": "Oak veneer 6 drawer chest of drawers, brushed brass handles",
            "output": {
                "category": "storage",
                "storage_type": "chest_of_drawers",
                "primary_material": "oak",
                "upholstery": None,
                "colour": "natural oak",
                "style": "contemporary",
                "room": "bedroom",
                "assembly_required": True,
                "brand": None,
            },
        },
    ),
)

# Generates a single free-text `product_type` — the specific type the warehouse
# leaves null. Deliberately distinct from the source columns (product_division /
# department / category / sub_category / range / type) so the LLM output is the
# product type inferred from the text, not a copy of the input taxonomy.
GENERIC = ExtractionSchema(
    domain="product type",
    description="the specific product type for a retail product",
    fields=(
        F(
            name="product_type",
            description="the most precise noun for what the product IS — its "
            "specific type, not its brand or attributes "
            "(e.g. running shoe, frying pan, wireless earbuds, office chair)",
        ),
    ),
    examples=(
        {
            "input": "Nike Air Zoom Pegasus 40 men's road running shoes, breathable mesh",
            "output": {"product_type": "running shoe"},
        },
        {
            "input": "Le Creuset cast iron frying pan, 28cm",
            "output": {"product_type": "frying pan"},
        },
    ),
)

SCHEMAS: dict[str, ExtractionSchema] = {
    "clothing": CLOTHING,
    "electronics": ELECTRONICS,
    "furniture": FURNITURE,
    "generic": GENERIC,
}


def get_schema(name: str) -> ExtractionSchema:
    try:
        return SCHEMAS[name]
    except KeyError:
        available = ", ".join(sorted(SCHEMAS))
        msg = f"Unknown domain {name!r}. Available: {available}"
        raise KeyError(msg) from None
