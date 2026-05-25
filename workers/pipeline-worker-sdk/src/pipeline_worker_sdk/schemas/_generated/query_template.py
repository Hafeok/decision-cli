"""Pydantic model for dec:QueryTemplate (structured-output schema)."""

# ============================================================
# GENERATED FILE — DO NOT EDIT BY HAND.
# Regenerate via:  uv run codegen
# ============================================================
# Source SHACL shape: workers/_shared/shapes/query-template.ttl
# Generator: pipeline-worker-sdk codegen (FT-085 / ADR-048)

from __future__ import annotations

from pydantic import BaseModel, Field


TARGET_CLASS_IRI = 'https://decision-cli.dev/ns#QueryTemplate'


class QueryTemplateSchema(BaseModel):
    """Pydantic schema for one dec:QueryTemplate artifact."""

    iri: str = Field(..., description="The artifact IRI.")
    querySpec: str = Field(..., description='dec:querySpec')
    version: str = Field(..., description='dec:version')
    queryLanguage: str | None = Field(default=None, description='dec:queryLanguage')
    addresses: list[str] = Field(default_factory=list, description="motivational edge: addresses")
    decomposesFrom: list[str] = Field(default_factory=list, description="motivational edge: decomposesFrom")

    model_config = {
        "extra": "forbid",
    }


__all__ = [
    "QueryTemplateSchema",
    "TARGET_CLASS_IRI",
]
