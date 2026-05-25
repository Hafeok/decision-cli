"""Read-only bundle accessor for dec:ADR artifacts."""

# ============================================================
# GENERATED FILE — DO NOT EDIT BY HAND.
# Regenerate via:  uv run codegen
# ============================================================
# Source SHACL shape: workers/_shared/shapes/adr.ttl
# Generator: pipeline-worker-sdk codegen (FT-085 / ADR-048)

from __future__ import annotations

from dataclasses import dataclass, field


TARGET_CLASS_IRI = 'https://decision-cli.dev/ns#ADR'


@dataclass(frozen=True)
class ADRAccessor:
    """Read-only view of one dec:ADR artifact in a bundle."""

    iri: str
    addresses: tuple[str, ...] = ()
    decidesFor: tuple[str, ...] = ()
    supersedes: tuple[str, ...] = ()


__all__ = [
    "ADRAccessor",
    "TARGET_CLASS_IRI",
]
