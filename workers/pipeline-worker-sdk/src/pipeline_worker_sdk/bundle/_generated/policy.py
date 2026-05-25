"""Read-only bundle accessor for dec:Policy artifacts."""

# ============================================================
# GENERATED FILE — DO NOT EDIT BY HAND.
# Regenerate via:  uv run codegen
# ============================================================
# Source SHACL shape: workers/_shared/shapes/policy.ttl
# Generator: pipeline-worker-sdk codegen (FT-085 / ADR-048)

from __future__ import annotations

from dataclasses import dataclass, field


TARGET_CLASS_IRI = 'https://decision-cli.dev/ns#Policy'


@dataclass(frozen=True)
class PolicyAccessor:
    """Read-only view of one dec:Policy artifact in a bundle."""

    iri: str
    addresses: tuple[str, ...] = ()
    decomposesFrom: tuple[str, ...] = ()


__all__ = [
    "PolicyAccessor",
    "TARGET_CLASS_IRI",
]
