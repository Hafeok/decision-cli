"""Side-channel emissions for emergent judgments + feedback artifacts (FT-082)."""

from .feedback import (
    FEEDBACK_CLASSES,
    FeedbackClass,
    FeedbackEmission,
    build_feedback_quads,
    emission_is_blocking,
    emission_target_role,
    mint_feedback_iri,
)
from .judgment import build_judgment_quads, mint_judgment_iri
from .vocab import (
    CLASS_BLOCKING_DEFAULTS,
    CLASS_TARGET_ROLE_DEFAULTS,
    DEC_NS,
    ORCHESTRATION_GRAPH,
    SEVERITIES,
)

__all__ = [
    "CLASS_BLOCKING_DEFAULTS",
    "CLASS_TARGET_ROLE_DEFAULTS",
    "DEC_NS",
    "FEEDBACK_CLASSES",
    "FeedbackClass",
    "FeedbackEmission",
    "ORCHESTRATION_GRAPH",
    "SEVERITIES",
    "build_feedback_quads",
    "build_judgment_quads",
    "emission_is_blocking",
    "emission_target_role",
    "mint_feedback_iri",
    "mint_judgment_iri",
]
