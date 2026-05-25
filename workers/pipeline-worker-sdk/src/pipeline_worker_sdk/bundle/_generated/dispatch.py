"""Read-only bundle accessor for dec:Dispatch artifacts."""

# ============================================================
# GENERATED FILE — DO NOT EDIT BY HAND.
# Regenerate via:  uv run codegen
# ============================================================
# Source SHACL shape: workers/_shared/shapes/dispatch.ttl
# Generator: pipeline-worker-sdk codegen (FT-085 / ADR-048)

from __future__ import annotations

from dataclasses import dataclass, field


TARGET_CLASS_IRI = 'https://decision-cli.dev/ns#Dispatch'


@dataclass(frozen=True)
class DispatchAccessor:
    """Read-only view of one dec:Dispatch artifact in a bundle."""

    iri: str
    # No SHACL-declared body fields or edges for this type.
    pass


__all__ = [
    "DispatchAccessor",
    "TARGET_CLASS_IRI",
]
