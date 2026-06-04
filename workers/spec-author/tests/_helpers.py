"""Synthetic bundle builders shared across the FT-129 spec-author TC suites."""

from __future__ import annotations

import json
from typing import Any

from spec_author.bundle import SpecAuthorInput


BUNDLE_HASH = "deadbeef" * 8  # 64 chars, looks-like-sha256

# Dispatcher-pinned identifiers used across the synthetic FT-129 bundles.
_FIXTURE_ENDPOINT = "anthropic"
_FIXTURE_MODEL_ID = "test-model-pinned-by-dispatcher"

# Required body-schema sections per ADR-047 / FT-055.
REQUIRED_H2 = ["Description", "Functional Specification", "Out of scope"]
REQUIRED_H3 = [
    "Inputs",
    "Outputs",
    "State",
    "Behaviour",
    "Invariants",
    "Error handling",
    "Boundaries",
]


CONFORMING_BODY = """\
## Description

A worker that does a thing. Mirrors FT-126's contract.

## Functional Specification

### Inputs

A Pydantic input bundle with everything the worker needs.

### Outputs

A Pydantic SpecProposal printed to stdout as JSON.

### State

None. Stateless; bundle in, proposal out.

### Behaviour

1. Parse the bundle.
2. Build the prompt.
3. Call the model with structured-output constraint.
4. Validate the response and echo `bundle_hash`.

### Invariants

- No filesystem writes outside stdout.
- No graph access.

### Error handling

- Bundle malformed → exit 2.
- Model call failure → exit 3.

### Boundaries

- In scope: the worker package and its tests.
- Out of scope: harness-side persistence.

## Out of scope

- Persistence (the harness owns writes).
- Multi-feature batch authoring.
"""


def _base_schema() -> dict:
    return {
        "required_h2_sections": REQUIRED_H2,
        "required_h3_subsections": REQUIRED_H3,
        "section_descriptions": {},
    }


def _base_authority() -> dict:
    return {
        "may_decide": ["section-internal-phrasing", "examples", "ordering"],
        "must_escalate": ["new ADRs", "scope changes"],
        "rationale": "spec-author may polish phrasing but must not invent ADRs.",
    }


def build_bundle_for_new() -> SpecAuthorInput:
    """TC-298 / TC-300 — well-formed request that should yield kind='new'."""
    return SpecAuthorInput.model_validate(
        {
            "endpoint": _FIXTURE_ENDPOINT,
            "model_id": _FIXTURE_MODEL_ID,
            "request": {
                "id": "REQ-001",
                "title": "Add a hypothetical worker",
                "body": (
                    "We need a hypothetical worker that takes inputs, "
                    "validates them, and emits a structured artifact. "
                    "Scope: one Python package, mirrors FT-126's shape. "
                    "Acceptance: the worker prints a SpecProposal JSON to "
                    "stdout and exits 0 on success. Boundaries: no graph "
                    "access from inside the worker."
                ),
                "source": "product request apply",
            },
            "feature_id_hint": "FT-XYZ",
            "body_schema": _base_schema(),
            "related_features": [
                {
                    "id": "FT-126",
                    "title": "tc-author worker",
                    "description": "Python tc-author worker package.",
                    "boundaries": "Out of scope: persistence of authored TCs.",
                }
            ],
            "central_adrs": [
                {
                    "id": "ADR-008",
                    "title": "Worker contract",
                    "scope": "cross-cutting",
                    "summary": "Workers are stateless bundle-in / artifact-out.",
                }
            ],
            "domain_registry": [
                {"name": "observability", "description": "Tracing and telemetry."},
                {"name": "api", "description": "External CLI surface."},
            ],
            "authority": _base_authority(),
            "bundle_hash": BUNDLE_HASH,
        }
    )


def build_bundle_for_gap() -> SpecAuthorInput:
    """TC-299 — under-specified request that should yield kind='gap'."""
    return SpecAuthorInput.model_validate(
        {
            "endpoint": _FIXTURE_ENDPOINT,
            "model_id": _FIXTURE_MODEL_ID,
            "request": {
                "id": "REQ-002",
                "title": "Build a thing",
                "body": (
                    "Build a thing. It must be synchronous and it must scale "
                    "to 10k concurrent users."
                ),
                "source": "product request apply",
            },
            "feature_id_hint": None,
            "body_schema": _base_schema(),
            "related_features": [],
            "central_adrs": [],
            "domain_registry": [],
            "authority": _base_authority(),
            "bundle_hash": BUNDLE_HASH,
        }
    )


def build_bundle_for_protocol_check() -> SpecAuthorInput:
    """TC-301 — any well-formed bundle, used for bundle-hash echo + write checks."""
    return SpecAuthorInput.model_validate(
        {
            "endpoint": _FIXTURE_ENDPOINT,
            "model_id": _FIXTURE_MODEL_ID,
            "request": {
                "id": "REQ-003",
                "title": "Protocol check",
                "body": "A trivial request for the protocol-check fixture.",
                "source": "test fixture",
            },
            "feature_id_hint": None,
            "body_schema": _base_schema(),
            "related_features": [],
            "central_adrs": [],
            "domain_registry": [],
            "authority": _base_authority(),
            "bundle_hash": "sha256:def4" + ("5" * 60),  # well-formed 64-char hash
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
    from spec_author import worker as worker_mod

    def boom(*args: Any, **kwargs: Any):  # noqa: ARG001
        raise AssertionError(
            "live model call escaped the mock — test must inject a caller"
        )

    monkeypatch.setattr(worker_mod, "_build_router_for_bundle", boom)
