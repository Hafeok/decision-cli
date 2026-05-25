"""Pydantic model for dec:Subscription (structured-output schema)."""

# ============================================================
# GENERATED FILE — DO NOT EDIT BY HAND.
# Regenerate via:  uv run codegen
# ============================================================
# Source SHACL shape: workers/_shared/shapes/subscription.ttl
# Generator: pipeline-worker-sdk codegen (FT-085 / ADR-048)

from __future__ import annotations

from pydantic import BaseModel, Field


TARGET_CLASS_IRI = 'https://decision-cli.dev/ns#Subscription'


class SubscriptionSchema(BaseModel):
    """Pydantic schema for one dec:Subscription artifact."""

    iri: str = Field(..., description="The artifact IRI.")
    motivatedBy: list[str] = Field(default_factory=list, description="motivational edge: motivatedBy")

    model_config = {
        "extra": "forbid",
    }


__all__ = [
    "SubscriptionSchema",
    "TARGET_CLASS_IRI",
]
