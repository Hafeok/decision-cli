"""Read-only bundle accessor for dec:QueryTemplate artifacts."""

# ============================================================
# GENERATED FILE — DO NOT EDIT BY HAND.
# Regenerate via:  uv run codegen
# ============================================================
# Source SHACL shape: workers/_shared/shapes/query-template.ttl
# Generator: pipeline-worker-sdk codegen (FT-085 / ADR-048)

from __future__ import annotations

from dataclasses import dataclass, field


TARGET_CLASS_IRI = 'https://decision-cli.dev/ns#QueryTemplate'


@dataclass(frozen=True)
class QueryTemplateAccessor:
    """Read-only view of one dec:QueryTemplate artifact in a bundle."""

    iri: str
    querySpec: str | None = None
    version: str | None = None
    queryLanguage: str | None = None
    addresses: tuple[str, ...] = ()
    decomposesFrom: tuple[str, ...] = ()


__all__ = [
    "QueryTemplateAccessor",
    "TARGET_CLASS_IRI",
]
