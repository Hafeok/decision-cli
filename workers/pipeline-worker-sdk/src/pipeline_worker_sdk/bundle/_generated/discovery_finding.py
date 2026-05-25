"""Read-only bundle accessor for dec:DiscoveryFinding artifacts."""

# ============================================================
# GENERATED FILE — DO NOT EDIT BY HAND.
# Regenerate via:  uv run codegen
# ============================================================
# Source SHACL shape: workers/_shared/shapes/discovery-finding.ttl
# Generator: pipeline-worker-sdk codegen (FT-085 / ADR-048)

from __future__ import annotations

from dataclasses import dataclass, field


TARGET_CLASS_IRI = 'https://decision-cli.dev/ns#DiscoveryFinding'


@dataclass(frozen=True)
class DiscoveryFindingAccessor:
    """Read-only view of one dec:DiscoveryFinding artifact in a bundle."""

    iri: str
    derivedFrom: tuple[str, ...] = ()


__all__ = [
    "DiscoveryFindingAccessor",
    "TARGET_CLASS_IRI",
]
