"""Pydantic model for dec:ADR (structured-output schema)."""

# ============================================================
# GENERATED FILE — DO NOT EDIT BY HAND.
# Regenerate via:  uv run codegen
# ============================================================
# Source SHACL shape: workers/_shared/shapes/adr.ttl
# Generator: pipeline-worker-sdk codegen (FT-085 / ADR-048)

from __future__ import annotations

from pydantic import BaseModel, Field


TARGET_CLASS_IRI = 'https://decision-cli.dev/ns#ADR'


class ADRSchema(BaseModel):
    """Pydantic schema for one dec:ADR artifact."""

    iri: str = Field(..., description="The artifact IRI.")
    addresses: list[str] = Field(default_factory=list, description="motivational edge: addresses")
    decidesFor: list[str] = Field(default_factory=list, description="motivational edge: decidesFor")
    supersedes: list[str] = Field(default_factory=list, description="motivational edge: supersedes")

    model_config = {
        "extra": "forbid",
    }


__all__ = [
    "ADRSchema",
    "TARGET_CLASS_IRI",
]
