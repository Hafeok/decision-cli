"""Pydantic model for dec:WorkerImage (structured-output schema)."""

# ============================================================
# GENERATED FILE — DO NOT EDIT BY HAND.
# Regenerate via:  uv run codegen
# ============================================================
# Source SHACL shape: workers/_shared/shapes/worker-image.ttl
# Generator: pipeline-worker-sdk codegen (FT-085 / ADR-048)

from __future__ import annotations

from pydantic import BaseModel, Field


TARGET_CLASS_IRI = 'https://decision-cli.dev/ns#WorkerImage'


class WorkerImageSchema(BaseModel):
    """Pydantic schema for one dec:WorkerImage artifact."""

    iri: str = Field(..., description="The artifact IRI.")
    addresses: list[str] = Field(default_factory=list, description="motivational edge: addresses")
    decomposesFrom: list[str] = Field(default_factory=list, description="motivational edge: decomposesFrom")

    model_config = {
        "extra": "forbid",
    }


__all__ = [
    "WorkerImageSchema",
    "TARGET_CLASS_IRI",
]
