"""Read-only bundle accessor for dec:Brief artifacts."""

# ============================================================
# GENERATED FILE — DO NOT EDIT BY HAND.
# Regenerate via:  uv run codegen
# ============================================================
# Source SHACL shape: workers/_shared/shapes/brief.ttl
# Generator: pipeline-worker-sdk codegen (FT-085 / ADR-048)

from __future__ import annotations

from dataclasses import dataclass, field


TARGET_CLASS_IRI = 'https://decision-cli.dev/ns#Brief'


@dataclass(frozen=True)
class BriefAccessor:
    """Read-only view of one dec:Brief artifact in a bundle."""

    iri: str
    goal: str | None = None
    premise: str | None = None
    successCriteria: str | None = None
    title: str | None = None
    acknowledges: tuple[str, ...] = ()
    decomposesInto: tuple[str, ...] = ()
    excludes: tuple[str, ...] = ()
    references: tuple[str, ...] = ()
    respondsTo: tuple[str, ...] = ()


__all__ = [
    "BriefAccessor",
    "TARGET_CLASS_IRI",
]
