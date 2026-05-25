"""Pydantic model for dec:Feature (structured-output schema)."""

# ============================================================
# GENERATED FILE — DO NOT EDIT BY HAND.
# Regenerate via:  uv run codegen
# ============================================================
# Source SHACL shape: workers/_shared/shapes/feature.ttl
# Generator: pipeline-worker-sdk codegen (FT-085 / ADR-048)

from __future__ import annotations

from pydantic import BaseModel, Field


TARGET_CLASS_IRI = 'https://decision-cli.dev/ns#Feature'


class FeatureSchema(BaseModel):
    """Pydantic schema for one dec:Feature artifact."""

    iri: str = Field(..., description="The artifact IRI.")
    addresses: list[str] = Field(default_factory=list, description="motivational edge: addresses")
    decomposesFrom: list[str] = Field(default_factory=list, description="motivational edge: decomposesFrom")
    originatedFrom: list[str] = Field(default_factory=list, description="motivational edge: originatedFrom")
    respondsTo: list[str] = Field(default_factory=list, description="motivational edge: respondsTo")

    model_config = {
        "extra": "forbid",
    }


__all__ = [
    "FeatureSchema",
    "TARGET_CLASS_IRI",
]
