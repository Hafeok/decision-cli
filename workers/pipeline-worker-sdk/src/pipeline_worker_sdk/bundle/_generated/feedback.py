"""Read-only bundle accessor for dec:Feedback artifacts."""

# ============================================================
# GENERATED FILE — DO NOT EDIT BY HAND.
# Regenerate via:  uv run codegen
# ============================================================
# Source SHACL shape: workers/_shared/shapes/feedback.ttl
# Generator: pipeline-worker-sdk codegen (FT-085 / ADR-048)

from __future__ import annotations

from dataclasses import dataclass, field


TARGET_CLASS_IRI = 'https://decision-cli.dev/ns#Feedback'


@dataclass(frozen=True)
class FeedbackAccessor:
    """Read-only view of one dec:Feedback artifact in a bundle."""

    iri: str
    observedIn: tuple[str, ...] = ()
    observedVia: tuple[str, ...] = ()
    producedBy: tuple[str, ...] = ()


__all__ = [
    "FeedbackAccessor",
    "TARGET_CLASS_IRI",
]
