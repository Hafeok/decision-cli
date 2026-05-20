"""TC-041: worker SDK emit_feedback produces a valid Feedback record.

Validates the FT-030 + FT-031 contract:

* The worker SDK exposes ``emit_feedback`` and the ``Bundle`` /
  ``Authority`` types backing the dispatch payload.
* A single ``emit_feedback`` call writes one ``__DEC_FEEDBACK__``-prefixed
  JSON line carrying the feedback class, severity, evidence, and any
  per-emission overrides. This is the "valid Feedback artifact in session
  telemetry" the harness ingests on the way to ``StreamWriter``.
* The emitted record round-trips into a ``FeedbackEmission`` (so the
  parsing path on the harness side is symmetric).
* The ``Bundle`` shape carries an FT-030 ``Authority`` field with
  ``may_decide`` / ``must_escalate`` lists workers consult to decide
  whether to escalate.
"""

from __future__ import annotations

import io
import json
import sys
from pathlib import Path

# Make the shared package importable without an editable install.
HERE = Path(__file__).resolve()
SRC = HERE.parent.parent / "src"
if str(SRC) not in sys.path:
    sys.path.insert(0, str(SRC))

import pytest  # noqa: E402

from _shared.bundle import Authority, Bundle, EscalationHint  # noqa: E402
from _shared.feedback import (  # noqa: E402
    FEEDBACK_RECORD_SENTINEL,
    FeedbackEmission,
    emit_feedback,
)


def _make_emission(**overrides) -> FeedbackEmission:
    defaults = dict(
        feedback_class="gap",
        severity="medium",
        evidence=(
            "feature_spec line 42 underspecifies the routing policy for the "
            "verifier subscription"
        ),
        recommendation="amend FT-022 to define the dispatch ordering",
    )
    defaults.update(overrides)
    return FeedbackEmission(**defaults)


def test_emit_feedback_writes_sentinel_prefixed_json_line() -> None:
    stream = io.StringIO()
    emission = _make_emission()
    line = emit_feedback(emission, stream=stream)

    written = stream.getvalue()
    assert written.endswith("\n")
    assert written.strip() == line
    assert line.startswith(f"{FEEDBACK_RECORD_SENTINEL} ")

    payload_json = line[len(FEEDBACK_RECORD_SENTINEL) + 1 :]
    payload = json.loads(payload_json)
    assert payload["feedback_class"] == "gap"
    assert payload["severity"] == "medium"
    assert "underspecifies" in payload["evidence"]
    assert payload["recommendation"].startswith("amend FT-022")


def test_emit_feedback_record_round_trips_into_emission() -> None:
    stream = io.StringIO()
    emit_feedback(
        _make_emission(
            feedback_class="contradiction",
            severity="high",
            target_role_override="architect",
        ),
        stream=stream,
    )
    line = stream.getvalue().strip()
    payload = json.loads(line[len(FEEDBACK_RECORD_SENTINEL) + 1 :])
    round_trip = FeedbackEmission.model_validate(payload)
    assert round_trip.feedback_class == "contradiction"
    assert round_trip.severity == "high"
    assert round_trip.target_role_override == "architect"
    assert "underspecifies" in round_trip.evidence


def test_emit_feedback_rejects_blank_evidence() -> None:
    with pytest.raises(Exception):
        _make_emission(evidence="   " * 8)


def test_emit_feedback_rejects_short_evidence() -> None:
    with pytest.raises(Exception):
        _make_emission(evidence="too short")


def test_emit_feedback_records_blocking_override_and_rationale() -> None:
    stream = io.StringIO()
    emit_feedback(
        _make_emission(
            feedback_class="defect",
            blocking=True,
            disposition_rationale=(
                "the defect, if shipped, breaks downstream dispatches"
            ),
        ),
        stream=stream,
    )
    payload = json.loads(stream.getvalue().strip()[len(FEEDBACK_RECORD_SENTINEL) + 1 :])
    assert payload["blocking"] is True
    assert "downstream" in payload["disposition_rationale"]


def test_bundle_exposes_authority_with_must_escalate_and_may_decide() -> None:
    # FT-030 bundle injection: workers consume the role's authority from
    # the dispatch payload via Bundle.authority. Verify the shape carries
    # the ADR-027 fields workers need to decide vs. escalate.
    authority = Authority(
        iri="https://decision-cli.dev/ns/authority/implementer",
        may_decide=[
            "code-style",
            "naming-within-feature",
            "internal-data-shape",
            "test-cases-for-this-feature",
        ],
        must_escalate=[
            "feature-spec-changes",
            "adr-changes",
            "cross-cutting-policy",
        ],
        escalate_via=[
            EscalationHint(
                category="feature-spec-changes",
                **{"class": "gap"},
                target_role="spec-author",
            )
        ],
        rationale="Implementer writes code from a feature_spec; structural changes escalate.",
    )
    bundle = Bundle(role_id="code-writer", authority=authority)
    assert bundle.authority is not None
    assert "code-style" in bundle.authority.may_decide
    for category in ("feature-spec-changes", "adr-changes", "cross-cutting-policy"):
        assert category in bundle.authority.must_escalate
    # may_decide and must_escalate must be disjoint (ADR-027 invariant).
    assert not set(bundle.authority.may_decide) & set(bundle.authority.must_escalate)
    # The escalateVia hint round-trips through the alias on `class`.
    hint = bundle.authority.escalate_via[0]
    assert hint.category == "feature-spec-changes"
    assert hint.feedback_class == "gap"
    assert hint.target_role == "spec-author"


def test_bundle_allows_absent_authority_for_legacy_stores() -> None:
    bundle = Bundle(role_id="code-writer", authority=None)
    assert bundle.authority is None


def test_emit_feedback_appends_to_existing_stream_content() -> None:
    # Workers usually print their normal WorkerResponse on stdout; the
    # feedback sentinel must coexist with that output unambiguously.
    stream = io.StringIO()
    stream.write("normal worker chatter\n")
    emit_feedback(_make_emission(), stream=stream)
    lines = stream.getvalue().splitlines()
    assert lines[0] == "normal worker chatter"
    assert lines[1].startswith(FEEDBACK_RECORD_SENTINEL)
