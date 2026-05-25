"""Pydantic model for dec:Dispatch (structured-output schema)."""

# ============================================================
# GENERATED FILE — DO NOT EDIT BY HAND.
# Regenerate via:  uv run codegen
# ============================================================
# Source SHACL shape: workers/_shared/shapes/dispatch.ttl
# Generator: pipeline-worker-sdk codegen (FT-085 / ADR-048)

from __future__ import annotations

from pydantic import BaseModel, Field


TARGET_CLASS_IRI = 'https://decision-cli.dev/ns#Dispatch'


class DispatchSchema(BaseModel):
    """Pydantic schema for one dec:Dispatch artifact."""

    iri: str = Field(..., description="The artifact IRI.")
    # No SHACL-declared body fields, edges, or motivational
    # alternatives for this type.
    pass

    model_config = {
        "extra": "forbid",
    }


__all__ = [
    "DispatchSchema",
    "TARGET_CLASS_IRI",
]
