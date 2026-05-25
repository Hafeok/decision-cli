"""Pydantic model for dec:Dependency (structured-output schema)."""

# ============================================================
# GENERATED FILE — DO NOT EDIT BY HAND.
# Regenerate via:  uv run codegen
# ============================================================
# Source SHACL shape: workers/_shared/shapes/dependency.ttl
# Generator: pipeline-worker-sdk codegen (FT-085 / ADR-048)

from __future__ import annotations

from pydantic import BaseModel, Field


TARGET_CLASS_IRI = 'https://decision-cli.dev/ns#Dependency'


class DependencySchema(BaseModel):
    """Pydantic schema for one dec:Dependency artifact."""

    iri: str = Field(..., description="The artifact IRI.")
    requiredBy: list[str] = Field(default_factory=list, description="motivational edge: requiredBy")

    model_config = {
        "extra": "forbid",
    }


__all__ = [
    "DependencySchema",
    "TARGET_CLASS_IRI",
]
