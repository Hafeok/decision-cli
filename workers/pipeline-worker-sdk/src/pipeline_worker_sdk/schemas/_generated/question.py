"""Pydantic model for dec:Question (structured-output schema)."""

# ============================================================
# GENERATED FILE — DO NOT EDIT BY HAND.
# Regenerate via:  uv run codegen
# ============================================================
# Source SHACL shape: workers/_shared/shapes/question.ttl
# Generator: pipeline-worker-sdk codegen (FT-085 / ADR-048)

from __future__ import annotations

from pydantic import BaseModel, Field


TARGET_CLASS_IRI = 'https://decision-cli.dev/ns#Question'


class QuestionSchema(BaseModel):
    """Pydantic schema for one dec:Question artifact."""

    iri: str = Field(..., description="The artifact IRI.")
    raisedBy: list[str] = Field(default_factory=list, description="motivational edge: raisedBy")
    raisedIn: list[str] = Field(default_factory=list, description="motivational edge: raisedIn")

    model_config = {
        "extra": "forbid",
    }


__all__ = [
    "QuestionSchema",
    "TARGET_CLASS_IRI",
]
