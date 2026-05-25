"""Pydantic model for dec:ConformanceAudit (structured-output schema)."""

# ============================================================
# GENERATED FILE — DO NOT EDIT BY HAND.
# Regenerate via:  uv run codegen
# ============================================================
# Source SHACL shape: workers/_shared/shapes/conformance-audit.ttl
# Generator: pipeline-worker-sdk codegen (FT-085 / ADR-048)

from __future__ import annotations

from pydantic import BaseModel, Field


TARGET_CLASS_IRI = 'https://decision-cli.dev/ns#ConformanceAudit'


class ConformanceAuditSchema(BaseModel):
    """Pydantic schema for one dec:ConformanceAudit artifact."""

    iri: str = Field(..., description="The artifact IRI.")
    audits: list[str] = Field(default_factory=list, description="motivational edge: audits")

    model_config = {
        "extra": "forbid",
    }


__all__ = [
    "ConformanceAuditSchema",
    "TARGET_CLASS_IRI",
]
