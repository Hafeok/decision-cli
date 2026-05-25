"""Read-only bundle accessor for dec:TC artifacts."""

# ============================================================
# GENERATED FILE — DO NOT EDIT BY HAND.
# Regenerate via:  uv run codegen
# ============================================================
# Source SHACL shape: workers/_shared/shapes/tc.ttl
# Generator: pipeline-worker-sdk codegen (FT-085 / ADR-048)

from __future__ import annotations

from dataclasses import dataclass, field


TARGET_CLASS_IRI = 'https://decision-cli.dev/ns#TC'


@dataclass(frozen=True)
class TCAccessor:
    """Read-only view of one dec:TC artifact in a bundle."""

    iri: str
    validates: tuple[str, ...] = ()


__all__ = [
    "TCAccessor",
    "TARGET_CLASS_IRI",
]
