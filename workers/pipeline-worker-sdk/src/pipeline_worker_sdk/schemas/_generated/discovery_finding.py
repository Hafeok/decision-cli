"""Pydantic model for dec:DiscoveryFinding (structured-output schema)."""

# ============================================================
# GENERATED FILE — DO NOT EDIT BY HAND.
# Regenerate via:  uv run codegen
# ============================================================
# Source SHACL shape: workers/_shared/shapes/discovery-finding.ttl
# Generator: pipeline-worker-sdk codegen (FT-085 / ADR-048)

from __future__ import annotations

from pydantic import BaseModel, Field


TARGET_CLASS_IRI = 'https://decision-cli.dev/ns#DiscoveryFinding'


class DiscoveryFindingSchema(BaseModel):
    """Pydantic schema for one dec:DiscoveryFinding artifact."""

    iri: str = Field(..., description="The artifact IRI.")
    derivedFrom: list[str] = Field(default_factory=list, description="motivational edge: derivedFrom")

    model_config = {
        "extra": "forbid",
    }


__all__ = [
    "DiscoveryFindingSchema",
    "TARGET_CLASS_IRI",
]
