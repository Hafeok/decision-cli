"""Synthetic bundle builders shared across the FT-130 adr-author TC suites."""

from __future__ import annotations

import json
from typing import Any

from adr_author.bundle import AdrAuthorInput

BUNDLE_HASH = "feedface" * 8  # 64 chars, looks-like-sha256

# Dispatcher-pinned identifiers used across the synthetic FT-130 bundles.
_FIXTURE_ENDPOINT = "anthropic"
_FIXTURE_MODEL_ID = "test-model-pinned-by-dispatcher"

# Required ADR body sections (Context, Decision, Rejected alternatives,
# Consequences, Status). The worker validates `new.body` against this list.
REQUIRED_ADR_H2 = [
    "Context",
    "Decision",
    "Rejected alternatives",
    "Consequences",
    "Status",
]


CONFORMING_ADR_BODY = """\
## Context

The planner can oscillate between two equivalent author dispatches when
no tie-breaker is declared. This needs a deliberate decision.

## Decision

Adopt deterministic ordering by (feature_id, role_name, dispatched_at).
Ties broken by sha256(bundle_hash) descending.

## Rejected alternatives

- **Random tie-break.** Reproducibility lost; debugging becomes harder.
- **First-come-first-served only.** Race conditions surface under load.

## Consequences

Oscillation traces become reproducible. The planner gains one extra
SPARQL clause per dispatch decision.

## Status

Proposed.
"""


def _base_body_schema() -> dict:
    return {
        "required_h2_sections": REQUIRED_ADR_H2,
        "section_descriptions": {},
    }


def _base_authority() -> dict:
    return {
        "may_decide": ["section-internal-phrasing", "alternative ordering"],
        "must_escalate": ["cross-cutting policy", "new artifact types"],
        "rationale": "adr-author may polish prose but must not invent policy.",
    }


def build_bundle_for_new() -> AdrAuthorInput:
    """TC-302 — preflight gap warranting a net-new ADR (no existing fits)."""
    return AdrAuthorInput.model_validate(
        {
            "endpoint": _FIXTURE_ENDPOINT,
            "model_id": _FIXTURE_MODEL_ID,
            "feature_id": "FT-901",
            "feature_spec": (
                "## Description\n\nA hypothetical planner feature that "
                "must decide which of two equivalent dispatches to run "
                "first. The spec needs an ADR to govern the ordering "
                "rule.\n\n"
            ),
            "preflight_gap": {
                "kind": "unacknowledged-adr",
                "adr_id": None,
                "domain": None,
                "severity": "error",
                "message": (
                    "no ADR governs planner dispatch oscillation handling; "
                    "FT-901 cannot ship without an ordering rule"
                ),
            },
            "central_adrs": [
                {
                    "id": "ADR-014",
                    "title": "Architectural Fitness Functions",
                    "scope": "cross-cutting",
                    "summary": (
                        "Code-quality rules live as cross-cutting ADRs with "
                        "linked TCs and shell-script enforcement."
                    ),
                }
            ],
            "adr_body_schema": _base_body_schema(),
            "domain_registry": [
                {"name": "observability", "description": "Tracing and telemetry."},
                {"name": "api", "description": "External CLI surface."},
            ],
            "authority": _base_authority(),
            "bundle_hash": BUNDLE_HASH,
        }
    )


def build_bundle_for_acknowledgement() -> AdrAuthorInput:
    """TC-303 — gap is governed by an existing ADR that wasn't linked."""
    return AdrAuthorInput.model_validate(
        {
            "endpoint": _FIXTURE_ENDPOINT,
            "model_id": _FIXTURE_MODEL_ID,
            "feature_id": "FT-902",
            "feature_spec": (
                "## Description\n\nA new fitness function that enforces a "
                "code-quality rule. Per the body, it slots cleanly under "
                "the existing cross-cutting rule framework.\n\n"
            ),
            "preflight_gap": {
                "kind": "unacknowledged-adr",
                "adr_id": "ADR-014",
                "domain": None,
                "severity": "warning",
                "message": (
                    "FT-902 introduces a cross-cutting check but does not "
                    "link the framing ADR (ADR-014)"
                ),
            },
            "central_adrs": [
                {
                    "id": "ADR-014",
                    "title": "Architectural Fitness Functions",
                    "scope": "cross-cutting",
                    "summary": (
                        "Code-quality rules live as cross-cutting ADRs with "
                        "linked TCs and shell-script enforcement; FT-902 "
                        "follows this pattern directly."
                    ),
                },
                {
                    "id": "ADR-013",
                    "title": "Code Structure and Quality Standards",
                    "scope": "cross-cutting",
                    "summary": "File-length and function-length limits.",
                },
            ],
            "adr_body_schema": _base_body_schema(),
            "domain_registry": [],
            "authority": _base_authority(),
            "bundle_hash": BUNDLE_HASH,
        }
    )


def build_bundle_for_bare_ack() -> AdrAuthorInput:
    """TC-304 — bundle shaped like an existing-ADR gap, used to test bare-ack defence."""
    return AdrAuthorInput.model_validate(
        {
            "endpoint": _FIXTURE_ENDPOINT,
            "model_id": _FIXTURE_MODEL_ID,
            "feature_id": "FT-903",
            "feature_spec": "## Description\n\nFeature body for TC-304.\n\n",
            "preflight_gap": {
                "kind": "unacknowledged-adr",
                "adr_id": "ADR-X",
                "domain": None,
                "severity": "warning",
                "message": "TC-304 fixture — bare-ack defence exercise.",
            },
            "central_adrs": [
                {
                    "id": "ADR-X",
                    "title": "An existing ADR",
                    "scope": "cross-cutting",
                    "summary": "Governs something relevant to FT-903.",
                }
            ],
            "adr_body_schema": _base_body_schema(),
            "domain_registry": [],
            "authority": _base_authority(),
            "bundle_hash": BUNDLE_HASH,
        }
    )


def build_bundle_for_gap_undefensible() -> AdrAuthorInput:
    """TC-305 — under-specified brief: no scope, no problem, no ADR candidates."""
    return AdrAuthorInput.model_validate(
        {
            "endpoint": _FIXTURE_ENDPOINT,
            "model_id": _FIXTURE_MODEL_ID,
            "feature_id": "FT-904",
            "feature_spec": "## Description\n\n(placeholder)\n\n",
            "preflight_gap": {
                "kind": "uncovered-domain",
                "adr_id": None,
                "domain": "unknown-thing",
                "severity": "warning",
                "message": "preflight surfaced an uncovered domain — too vague to action",
            },
            "central_adrs": [],
            "adr_body_schema": _base_body_schema(),
            "domain_registry": [],
            "authority": _base_authority(),
            "bundle_hash": BUNDLE_HASH,
        }
    )


def make_caller(*responses):
    """Return a ModelCaller that yields each response in order, counting calls."""
    calls: list[tuple[str, str, str, int]] = []
    queue = list(responses)

    def caller(system: str, user: str, model_id: str, max_tokens: int):
        calls.append((system, user, model_id, max_tokens))
        if not queue:
            raise AssertionError("ModelCaller invoked more times than responses provided")
        head = queue.pop(0)
        raw = head if isinstance(head, str) else json.dumps(head)
        return raw, 100, 50

    caller.calls = calls  # type: ignore[attr-defined]
    return caller


def assert_no_anthropic_attempt(monkeypatch):
    """Replace the live router builder with a sentinel that fails if invoked."""
    from adr_author import worker as worker_mod

    def boom(*args: Any, **kwargs: Any):  # noqa: ARG001
        raise AssertionError(
            "live model call escaped the mock — test must inject a caller"
        )

    monkeypatch.setattr(worker_mod, "_build_router_for_bundle", boom)
