"""Pydantic model for dec:Acknowledgement (structured-output schema)."""

# ============================================================
# GENERATED FILE — DO NOT EDIT BY HAND.
# Regenerate via:  uv run codegen
# ============================================================
# Source SHACL shape: workers/_shared/shapes/acknowledgement.ttl
# Generator: pipeline-worker-sdk codegen (FT-085 / ADR-048)

from __future__ import annotations

from pydantic import BaseModel, Field


TARGET_CLASS_IRI = 'https://decision-cli.dev/ns#Acknowledgement'


class AcknowledgementSchema(BaseModel):
    """Pydantic schema for one dec:Acknowledgement artifact."""

    iri: str = Field(..., description="The artifact IRI.")
    motivatedBy: list[str] = Field(default_factory=list, description="motivational edge: motivatedBy")

    model_config = {
        "extra": "forbid",
    }


__all__ = [
    "AcknowledgementSchema",
    "TARGET_CLASS_IRI",
]
