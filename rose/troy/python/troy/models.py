"""The ergonomic 'generated types' layer.

In production you would GENERATE this from the OpenAPI schema:

    datamodel-code-generator --input ../registry/openapi/ingest.yaml \\
        --input-file-type openapi --output troy/models.py

It is hand-written here to keep the demo dependency-light, but it mirrors the
schema exactly. The point: once past the wall, code works with typed, validated
objects — `IngestCustomer` — not raw dicts. Pydantic re-checks the same
constraints, so even code paths that skip the boundary stay safe.
"""

from __future__ import annotations

from enum import StrEnum

from pydantic import BaseModel, ConfigDict, Field


class Tier(StrEnum):
    free = "free"
    starter = "starter"
    professional = "professional"
    enterprise = "enterprise"


class IngestCustomer(BaseModel):
    model_config = ConfigDict(extra="forbid")  # mirrors additionalProperties: false

    customer_id: str = Field(pattern=r"^cust_[a-z0-9]+$")
    email: str = Field(max_length=254)
    tier: Tier
    annual_revenue: float | None = Field(default=None, ge=0, le=1e12)
    signup_country: str | None = Field(default=None, pattern=r"^[A-Z]{2}$")
