"""Read-only bundle accessor for dec:Acknowledgement artifacts."""

# ============================================================
# GENERATED FILE — DO NOT EDIT BY HAND.
# Regenerate via:  uv run codegen
# ============================================================
# Source SHACL shape: workers/_shared/shapes/acknowledgement.ttl
# Generator: pipeline-worker-sdk codegen (FT-085 / ADR-048)

from __future__ import annotations

from dataclasses import dataclass, field


TARGET_CLASS_IRI = 'https://decision-cli.dev/ns#Acknowledgement'


@dataclass(frozen=True)
class AcknowledgementAccessor:
    """Read-only view of one dec:Acknowledgement artifact in a bundle."""

    iri: str
    motivatedBy: tuple[str, ...] = ()


__all__ = [
    "AcknowledgementAccessor",
    "TARGET_CLASS_IRI",
]
