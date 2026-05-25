"""Read-only bundle accessor for dec:ConformanceAudit artifacts."""

# ============================================================
# GENERATED FILE — DO NOT EDIT BY HAND.
# Regenerate via:  uv run codegen
# ============================================================
# Source SHACL shape: workers/_shared/shapes/conformance-audit.ttl
# Generator: pipeline-worker-sdk codegen (FT-085 / ADR-048)

from __future__ import annotations

from dataclasses import dataclass, field


TARGET_CLASS_IRI = 'https://decision-cli.dev/ns#ConformanceAudit'


@dataclass(frozen=True)
class ConformanceAuditAccessor:
    """Read-only view of one dec:ConformanceAudit artifact in a bundle."""

    iri: str
    audits: tuple[str, ...] = ()


__all__ = [
    "ConformanceAuditAccessor",
    "TARGET_CLASS_IRI",
]
