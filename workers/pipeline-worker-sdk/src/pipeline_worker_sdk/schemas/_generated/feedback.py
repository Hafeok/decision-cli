"""Pydantic model for dec:Feedback (structured-output schema)."""

# ============================================================
# GENERATED FILE — DO NOT EDIT BY HAND.
# Regenerate via:  uv run codegen
# ============================================================
# Source SHACL shape: workers/_shared/shapes/feedback.ttl
# Generator: pipeline-worker-sdk codegen (FT-085 / ADR-048)

from __future__ import annotations

from pydantic import BaseModel, Field


TARGET_CLASS_IRI = 'https://decision-cli.dev/ns#Feedback'


class FeedbackSchema(BaseModel):
    """Pydantic schema for one dec:Feedback artifact."""

    iri: str = Field(..., description="The artifact IRI.")
    observedIn: list[str] = Field(default_factory=list, description="motivational edge: observedIn")
    observedVia: list[str] = Field(default_factory=list, description="motivational edge: observedVia")
    producedBy: list[str] = Field(default_factory=list, description="motivational edge: producedBy")

    model_config = {
        "extra": "forbid",
    }


__all__ = [
    "FeedbackSchema",
    "TARGET_CLASS_IRI",
]
