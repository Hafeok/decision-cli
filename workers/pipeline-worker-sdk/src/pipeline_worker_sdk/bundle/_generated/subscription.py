"""Read-only bundle accessor for dec:Subscription artifacts."""

# ============================================================
# GENERATED FILE — DO NOT EDIT BY HAND.
# Regenerate via:  uv run codegen
# ============================================================
# Source SHACL shape: workers/_shared/shapes/subscription.ttl
# Generator: pipeline-worker-sdk codegen (FT-085 / ADR-048)

from __future__ import annotations

from dataclasses import dataclass, field


TARGET_CLASS_IRI = 'https://decision-cli.dev/ns#Subscription'


@dataclass(frozen=True)
class SubscriptionAccessor:
    """Read-only view of one dec:Subscription artifact in a bundle."""

    iri: str
    motivatedBy: tuple[str, ...] = ()


__all__ = [
    "SubscriptionAccessor",
    "TARGET_CLASS_IRI",
]
