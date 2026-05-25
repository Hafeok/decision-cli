"""Pydantic model for dec:Brief (structured-output schema)."""

# ============================================================
# GENERATED FILE — DO NOT EDIT BY HAND.
# Regenerate via:  uv run codegen
# ============================================================
# Source SHACL shape: workers/_shared/shapes/brief.ttl
# Generator: pipeline-worker-sdk codegen (FT-085 / ADR-048)

from __future__ import annotations

from pydantic import BaseModel, Field


TARGET_CLASS_IRI = 'https://decision-cli.dev/ns#Brief'


class BriefSchema(BaseModel):
    """Pydantic schema for one dec:Brief artifact."""

    iri: str = Field(..., description="The artifact IRI.")
    goal: str = Field(..., description='dec:goal')
    premise: str = Field(..., description='dec:premise')
    successCriteria: str = Field(..., description='dec:successCriteria')
    title: str = Field(..., description='dec:title')
    acknowledges: list[str] = Field(default_factory=list, description='dec:acknowledges')
    decomposesInto: list[str] = Field(default_factory=list, description='dec:decomposesInto')
    excludes: list[str] = Field(default_factory=list, description='dec:excludes')
    references: list[str] = Field(default_factory=list, description='dec:references')
    respondsTo: list[str] = Field(default_factory=list, description="motivational edge: respondsTo")

    model_config = {
        "extra": "forbid",
    }


__all__ = [
    "BriefSchema",
    "TARGET_CLASS_IRI",
]
