"""Read-only bundle accessor for dec:Feature artifacts."""

# ============================================================
# GENERATED FILE — DO NOT EDIT BY HAND.
# Regenerate via:  uv run codegen
# ============================================================
# Source SHACL shape: workers/_shared/shapes/feature.ttl
# Generator: pipeline-worker-sdk codegen (FT-085 / ADR-048)

from __future__ import annotations

from dataclasses import dataclass, field


TARGET_CLASS_IRI = 'https://decision-cli.dev/ns#Feature'


@dataclass(frozen=True)
class FeatureAccessor:
    """Read-only view of one dec:Feature artifact in a bundle."""

    iri: str
    addresses: tuple[str, ...] = ()
    decomposesFrom: tuple[str, ...] = ()
    originatedFrom: tuple[str, ...] = ()
    respondsTo: tuple[str, ...] = ()


__all__ = [
    "FeatureAccessor",
    "TARGET_CLASS_IRI",
]
