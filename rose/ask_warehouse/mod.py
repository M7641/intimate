from pathlib import Path

MODULE_DIR = Path(__file__).parent
SCHEMA_DIR = MODULE_DIR / "schema"
VALIDATION_DIR = MODULE_DIR / "validation"
GLOSSARY_PATH = MODULE_DIR / "glossary.yaml"
FEW_SHOT_PATH = MODULE_DIR / "few_shot.yaml"
CURATED_TABLES_PATH = SCHEMA_DIR / "curated_tables.yaml"
GOLDEN_SET_PATH = VALIDATION_DIR / "golden_set.yaml"
