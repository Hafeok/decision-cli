"""Read-only bundle accessor for dec:Question artifacts."""

# ============================================================
# GENERATED FILE — DO NOT EDIT BY HAND.
# Regenerate via:  uv run codegen
# ============================================================
# Source SHACL shape: workers/_shared/shapes/question.ttl
# Generator: pipeline-worker-sdk codegen (FT-085 / ADR-048)

from __future__ import annotations

from dataclasses import dataclass, field


TARGET_CLASS_IRI = 'https://decision-cli.dev/ns#Question'


@dataclass(frozen=True)
class QuestionAccessor:
    """Read-only view of one dec:Question artifact in a bundle."""

    iri: str
    raisedBy: tuple[str, ...] = ()
    raisedIn: tuple[str, ...] = ()


__all__ = [
    "QuestionAccessor",
    "TARGET_CLASS_IRI",
]
