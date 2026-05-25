"""IRI vocabulary used by side-channel emissions (mirrors core::vocab::feedback)."""

from __future__ import annotations

from typing import Final

# RDF/RDFS substrate
RDF_TYPE: Final[str] = "http://www.w3.org/1999/02/22-rdf-syntax-ns#type"

# `dec:` namespace (matches `crates/decision-cli/src/core/vocab/feedback.rs`)
DEC_NS: Final[str] = "https://decision-cli.dev/ns#"

# Feedback artifact vocabulary (FT-026 / ADR-022)
DEC_FEEDBACK: Final[str] = f"{DEC_NS}Feedback"
DEC_FEEDBACK_CLASS: Final[str] = f"{DEC_NS}feedbackClass"
DEC_SEVERITY: Final[str] = f"{DEC_NS}severity"
DEC_EVIDENCE: Final[str] = f"{DEC_NS}evidence"
DEC_RECOMMENDATION: Final[str] = f"{DEC_NS}recommendation"
DEC_LIFECYCLE_STATE: Final[str] = f"{DEC_NS}lifecycleState"
DEC_SOURCE_SESSION: Final[str] = f"{DEC_NS}sourceSession"
DEC_TARGET_ROLE: Final[str] = f"{DEC_NS}targetRole"
DEC_DISPOSITION_OVERRIDE: Final[str] = f"{DEC_NS}dispositionOverride"
DEC_DISPOSITION_RATIONALE: Final[str] = f"{DEC_NS}dispositionRationale"

# Emergent judgment vocabulary (FT-082)
DEC_EMERGENT_JUDGMENT: Final[str] = f"{DEC_NS}EmergentJudgment"
DEC_DECISION: Final[str] = f"{DEC_NS}decision"
DEC_RATIONALE: Final[str] = f"{DEC_NS}rationale"
DEC_RECORDED_AT: Final[str] = f"{DEC_NS}recordedAt"

# Default named graph for emitted side-channel quads (matches the harness's
# orchestration named graph; the GraphWriter chokepoint validates anything
# the worker hands it, regardless of the graph the worker picks).
ORCHESTRATION_GRAPH: Final[str] = f"{DEC_NS}orchestration"

# Initial lifecycle state for a freshly-emitted feedback (ADR-024).
FEEDBACK_STATE_PRODUCED: Final[str] = "produced"

# ADR-023 controlled feedback class vocabulary.
FEEDBACK_CLASSES: Final[tuple[str, ...]] = (
    "gap",
    "contradiction",
    "unimplementable",
    "scope-issue",
    "defect",
    "capability-request",
)

# Allowed severity strings: low/medium/high mirrors the legacy worker SDK,
# info/warning/error mirrors the harness-side `Severity` enum. Both are
# accepted on the wire so workers can emit either vocabulary.
SEVERITIES: Final[tuple[str, ...]] = (
    "info",
    "warning",
    "error",
    "low",
    "medium",
    "high",
)

# Per-class default blocking disposition (ADR-023 + ADR-025).
CLASS_BLOCKING_DEFAULTS: Final[dict[str, bool]] = {
    "gap": True,
    "contradiction": True,
    "unimplementable": True,
    "scope-issue": False,
    "defect": False,
    "capability-request": False,
}

# Per-class default routing target role (ADR-026 routing table).
CLASS_TARGET_ROLE_DEFAULTS: Final[dict[str, str]] = {
    "gap": "spec-author",
    "contradiction": "architect",
    "unimplementable": "spec-author",
    "scope-issue": "slice-curator",
    "defect": "verifier",
    "capability-request": "architect",
}


__all__ = [
    "CLASS_BLOCKING_DEFAULTS",
    "CLASS_TARGET_ROLE_DEFAULTS",
    "DEC_DECISION",
    "DEC_DISPOSITION_OVERRIDE",
    "DEC_DISPOSITION_RATIONALE",
    "DEC_EMERGENT_JUDGMENT",
    "DEC_EVIDENCE",
    "DEC_FEEDBACK",
    "DEC_FEEDBACK_CLASS",
    "DEC_LIFECYCLE_STATE",
    "DEC_NS",
    "DEC_RATIONALE",
    "DEC_RECOMMENDATION",
    "DEC_RECORDED_AT",
    "DEC_SEVERITY",
    "DEC_SOURCE_SESSION",
    "DEC_TARGET_ROLE",
    "FEEDBACK_CLASSES",
    "FEEDBACK_STATE_PRODUCED",
    "ORCHESTRATION_GRAPH",
    "RDF_TYPE",
    "SEVERITIES",
]
