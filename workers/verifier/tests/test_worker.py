"""Worker-level tests: injected ModelCaller + retry behaviour."""

from __future__ import annotations

import json

import pytest

from verifier.bundle import VerifierInput
from verifier.worker import (
    DEFAULT_MODEL_ID,
    MODEL_ENV_VAR,
    VerifierError,
    _extract_json_object,
    resolve_model_id,
    run_verifier,
)


def _bundle(**overrides) -> VerifierInput:
    base = {
        "dispatch_id": "urn:dec:dispatch:1",
        "dispatch_group": "urn:dec:group:1",
        "interpretation_session": "urn:dec:session:interp-1",
        "action_session": "urn:dec:session:action-1",
        "feature_id": "FT-013",
        "feature_spec": "# FT-013\n\nspec body",
        "produced_artifact": "diff: file foo.rs +1 -0",
        "bundle_hash": "deadbeef" * 8,
        "in_stream": "https://decision-cli.dev/stream/test",
        "relevant_tcs": [{"id": "TC-013", "type": "invariant", "body": "tc body"}],
        "relevant_adrs": [{"id": "ADR-008", "scope": "cross-cutting", "body": "adr body"}],
    }
    base.update(overrides)
    return VerifierInput.model_validate(base)


def _make_caller(*responses: dict | str):
    """Return a ModelCaller that yields the supplied responses in order."""
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


# ---------------------------------------------------------------------------
# Happy path
# ---------------------------------------------------------------------------


def test_run_verifier_returns_approved_on_clean_response() -> None:
    caller = _make_caller(
        {
            "verdict": "approved",
            "rationale": "Produced CodeChange satisfies TC-013 and ADR-008.",
            "violates": [],
        }
    )
    result = run_verifier(_bundle(), caller=caller)
    assert result.verdict.verdict == "approved"
    assert result.telemetry.attempts == 1
    assert result.telemetry.input_tokens == 100
    assert result.telemetry.output_tokens == 50
    assert len(caller.calls) == 1


def test_run_verifier_handles_rejected_response() -> None:
    caller = _make_caller(
        {
            "verdict": "rejected",
            "rationale": "Action did not write any file under workers/code-writer/.",
            "violates": ["TC-013"],
        }
    )
    result = run_verifier(_bundle(), caller=caller)
    assert result.verdict.verdict == "rejected"
    assert result.verdict.violates == ["TC-013"]


def test_run_verifier_handles_amendment_required() -> None:
    caller = _make_caller(
        {
            "verdict": "amendment-required",
            "rationale": "Missing dec:inStream tag; the rest satisfies the spec.",
            "violates": ["ADR-005"],
            "amendment_guidance": "Add dec:inStream to every persisted artifact.",
        }
    )
    result = run_verifier(_bundle(), caller=caller)
    assert result.verdict.verdict == "amendment-required"
    assert result.verdict.amendment_guidance is not None


# ---------------------------------------------------------------------------
# Re-prompt behaviour
# ---------------------------------------------------------------------------


def test_run_verifier_retries_once_when_first_response_invalid() -> None:
    caller = _make_caller(
        # First response: invalid verdict value.
        {"verdict": "maybe", "rationale": "rationale long enough for SHACL"},
        # Second response: valid.
        {
            "verdict": "approved",
            "rationale": "On second look the produced artifact is fine.",
            "violates": [],
        },
    )
    result = run_verifier(_bundle(), caller=caller)
    assert result.verdict.verdict == "approved"
    assert result.telemetry.attempts == 2
    assert len(caller.calls) == 2
    # Second call must include the retry nudge in the user message.
    second_user = caller.calls[1][1]
    assert "did not satisfy the VerificationVerdict schema" in second_user


def test_run_verifier_fails_after_two_bad_responses() -> None:
    caller = _make_caller(
        {"verdict": "maybe", "rationale": "rationale long enough"},
        {"verdict": "also-bad", "rationale": "rationale long enough"},
    )
    with pytest.raises(VerifierError):
        run_verifier(_bundle(), caller=caller)


def test_run_verifier_retries_when_rationale_too_short() -> None:
    caller = _make_caller(
        {"verdict": "approved", "rationale": "ok"},
        {
            "verdict": "approved",
            "rationale": "After retry the rationale meets the 20-char floor.",
            "violates": [],
        },
    )
    result = run_verifier(_bundle(), caller=caller)
    assert result.telemetry.attempts == 2
    assert result.verdict.verdict == "approved"


def test_run_verifier_retries_when_rejected_lacks_citation() -> None:
    caller = _make_caller(
        # First response: rejected with no violates → ADR-018 conditional constraint.
        {
            "verdict": "rejected",
            "rationale": "Spec says one thing, artifact does another.",
            "violates": [],
        },
        {
            "verdict": "rejected",
            "rationale": "TC-013 is not satisfied; the produced file is absent.",
            "violates": ["TC-013"],
        },
    )
    result = run_verifier(_bundle(), caller=caller)
    assert result.telemetry.attempts == 2
    assert result.verdict.violates == ["TC-013"]


# ---------------------------------------------------------------------------
# Empty / malformed model output
# ---------------------------------------------------------------------------


def test_run_verifier_fails_on_empty_model_output() -> None:
    caller = _make_caller("", "")
    with pytest.raises(VerifierError):
        run_verifier(_bundle(), caller=caller)


def test_run_verifier_extracts_json_from_prose_wrapper() -> None:
    prose = (
        "Sure, here is the verdict:\n\n"
        '{"verdict": "approved", '
        '"rationale": "Produced artifact satisfies the listed criteria.", '
        '"violates": []}'
        "\n\nThanks!"
    )
    caller = _make_caller(prose)
    result = run_verifier(_bundle(), caller=caller)
    assert result.verdict.verdict == "approved"


# ---------------------------------------------------------------------------
# Model id resolution
# ---------------------------------------------------------------------------


def test_resolve_model_id_uses_bundle_pin() -> None:
    b = _bundle(model_id="claude-opus-4-1-fictitious")
    assert resolve_model_id(b) == "claude-opus-4-1-fictitious"


def test_resolve_model_id_falls_back_to_default(monkeypatch) -> None:
    monkeypatch.delenv(MODEL_ENV_VAR, raising=False)
    assert resolve_model_id(_bundle()) == DEFAULT_MODEL_ID


def test_resolve_model_id_uses_env_when_bundle_is_default(monkeypatch) -> None:
    monkeypatch.setenv(MODEL_ENV_VAR, "claude-haiku-stand-in")
    assert resolve_model_id(_bundle()) == "claude-haiku-stand-in"


# ---------------------------------------------------------------------------
# JSON extractor
# ---------------------------------------------------------------------------


def test_extract_json_object_plain() -> None:
    assert _extract_json_object('{"a": 1}') == {"a": 1}


def test_extract_json_object_with_prefix() -> None:
    text = 'prefix text\n{"a": 1, "b": "x"}\nsuffix'
    assert _extract_json_object(text) == {"a": 1, "b": "x"}


def test_extract_json_object_returns_none_for_no_object() -> None:
    assert _extract_json_object("no json here") is None


def test_extract_json_object_handles_nested() -> None:
    assert _extract_json_object('garbage {"a": {"b": 2}} trailing') == {"a": {"b": 2}}
