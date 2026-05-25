"""Pydantic model for dec:TC (structured-output schema)."""

# ============================================================
# GENERATED FILE — DO NOT EDIT BY HAND.
# Regenerate via:  uv run codegen
# ============================================================
# Source SHACL shape: workers/_shared/shapes/tc.ttl
# Generator: pipeline-worker-sdk codegen (FT-085 / ADR-048)

from __future__ import annotations

from pydantic import BaseModel, Field


TARGET_CLASS_IRI = 'https://decision-cli.dev/ns#TC'


class TCSchema(BaseModel):
    """Pydantic schema for one dec:TC artifact."""

    iri: str = Field(..., description="The artifact IRI.")
    validates: list[str] = Field(default_factory=list, description="motivational edge: validates")

    model_config = {
        "extra": "forbid",
    }


__all__ = [
    "TCSchema",
    "TARGET_CLASS_IRI",
]
