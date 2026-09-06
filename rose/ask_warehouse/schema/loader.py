"""Typed loader for ``curated_tables.yaml``.

The YAML is the source of truth (a domain expert edits it directly);
this module just gives us a frozen Pydantic shape to work with so the
prompt renderer and the safety allowlist read the same data.
"""

import yaml
from pydantic import BaseModel, Field

from common_py.ask_warehouse.mod import CURATED_TABLES_PATH


class CuratedColumn(BaseModel, frozen=True, extra="forbid"):
    type: str
    desc: str


class CuratedForeignKey(BaseModel, frozen=True, extra="forbid"):
    col: str
    ref: str


class CuratedTable(BaseModel, frozen=True, extra="forbid"):
    full_name: str
    grain: str
    primary_key: list[str] = Field(default_factory=list)
    columns: dict[str, CuratedColumn]
    foreign_keys: list[CuratedForeignKey] = Field(default_factory=list)
    typical_filters: list[str] = Field(default_factory=list)


class CuratedSchema(BaseModel, frozen=True, extra="forbid"):
    tables: dict[str, CuratedTable]

    def allowed_table_names(self) -> frozenset[str]:
        """Set of short names + dotted full_names the LLM may reference.

        ``safety.py`` consults this to reject queries that hallucinate
        a table outside the curated set.
        """
        names: set[str] = set()
        for short, table in self.tables.items():
            names.add(short.lower())
            names.add(table.full_name.lower())
            names.add(table.full_name.split(".")[-1].lower())
        return frozenset(names)


def load_curated_schema(path: object = None) -> CuratedSchema:
    """Load and validate the curated schema YAML.

    ``path`` defaults to ``CURATED_TABLES_PATH``; override in tests.
    """
    target = path if path is not None else CURATED_TABLES_PATH
    with open(target) as f:
        raw = yaml.safe_load(f)
    return CuratedSchema.model_validate(raw)
