"""Read-only bundle accessor for dec:Dependency artifacts."""

# ============================================================
# GENERATED FILE — DO NOT EDIT BY HAND.
# Regenerate via:  uv run codegen
# ============================================================
# Source SHACL shape: workers/_shared/shapes/dependency.ttl
# Generator: pipeline-worker-sdk codegen (FT-085 / ADR-048)

from __future__ import annotations

from dataclasses import dataclass, field


TARGET_CLASS_IRI = 'https://decision-cli.dev/ns#Dependency'


@dataclass(frozen=True)
class DependencyAccessor:
    """Read-only view of one dec:Dependency artifact in a bundle."""

    iri: str
    requiredBy: tuple[str, ...] = ()


__all__ = [
    "DependencyAccessor",
    "TARGET_CLASS_IRI",
]
