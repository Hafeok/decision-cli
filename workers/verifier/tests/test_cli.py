"""End-to-end CLI tests: stdin VerifierInput → stdout VerificationVerdict.

Exercises ADR-008 §"Worker exits 0 iff it produced a Pydantic-validated
VerificationVerdict on stdout" via the package's `__main__` entry point.
"""

from __future__ import annotations

import io
import json

import pytest

from verifier import __main__ as cli
from verifier.output import VerificationVerdict


def _bundle_blob(**overrides) -> dict:
    base = {
        "dispatch_id": "urn:dec:dispatch:1",
        "dispatch_group": "urn:dec:group:1",
        "interpretation_session": "urn:dec:session:interp-1",
        "action_session": "urn:dec:session:action-1",
        "feature_id": "FT-013",
        "feature_spec": "spec body long enough to be meaningful",
        "produced_artifact": "diff: foo.rs +1 -0",
        "bundle_hash": "deadbeef" * 8,
        "in_stream": "https://decision-cli.dev/stream/test",
        "relevant_tcs": [{"id": "TC-013"}],
        "relevant_adrs": [{"id": "ADR-008"}],
    }
    base.update(overrides)
    return base


def _run_cli(blob: dict | str | None, monkeypatch, *args, stub: bool = True) -> tuple[int, str, str]:
    """Drive the CLI with stdin = blob, capturing stdout/stderr."""
    if blob is None:
        raw = ""
    elif isinstance(blob, str):
        raw = blob
    else:
        raw = json.dumps(blob)
    stdin = io.StringIO(raw)
    stdout = io.StringIO()
    stderr = io.StringIO()
    monkeypatch.setattr("sys.stdin", stdin)
    monkeypatch.setattr("sys.stdout", stdout)
    monkeypatch.setattr("sys.stderr", stderr)
    if stub:
        monkeypatch.setenv("VERIFIER_STUB", "1")
    else:
        monkeypatch.delenv("VERIFIER_STUB", raising=False)
    code = cli.main(list(args))
    return code, stdout.getvalue(), stderr.getvalue()


def test_cli_round_trip_stub_mode(monkeypatch) -> None:
    code, out, err = _run_cli(_bundle_blob(), monkeypatch)
    assert code == 0, f"expected exit 0, got {code}; stderr={err}"
    verdict = VerificationVerdict.model_validate_json(out.strip())
    assert verdict.verdict == "approved"
    assert err == "" or err.endswith("\n")


def test_cli_empty_stdin_is_parse_error(monkeypatch) -> None:
    code, out, err = _run_cli(None, monkeypatch)
    assert code == 1
    assert out == ""
    assert "bundle parse error" in err


def test_cli_invalid_json_is_parse_error(monkeypatch) -> None:
    code, out, err = _run_cli("{not json", monkeypatch)
    assert code == 1
    assert "bundle parse error" in err


def test_cli_missing_required_field_is_parse_error(monkeypatch) -> None:
    blob = _bundle_blob()
    blob.pop("feature_id")
    code, out, err = _run_cli(blob, monkeypatch)
    assert code == 1
    assert "bundle parse error" in err


def test_cli_telemetry_flag_emits_telemetry_to_stderr(monkeypatch) -> None:
    code, out, err = _run_cli(_bundle_blob(), monkeypatch, "--include-telemetry")
    assert code == 0
    # Stdout must remain a single VerificationVerdict JSON object.
    VerificationVerdict.model_validate_json(out.strip())
    # Stderr must contain a JSON telemetry object.
    parsed = json.loads(err.strip().splitlines()[-1])
    assert parsed["exit_reason"] == "ok"
    assert "model_id" in parsed
    assert parsed["attempts"] >= 1


def test_cli_model_call_failure_exits_one(monkeypatch) -> None:
    """When the worker raises VerifierError after retry, exit 1 with stderr."""
    from verifier import __main__ as cli_mod

    def boom(bundle, caller=None):  # noqa: ARG001
        from verifier.worker import VerifierError

        raise VerifierError("synthetic model failure")

    monkeypatch.setattr(cli_mod, "run_verifier", boom)
    code, out, err = _run_cli(_bundle_blob(), monkeypatch)
    assert code == 1
    assert out == ""
    assert "model call failed" in err


def test_cli_short_rationale_emits_specific_diagnostic(monkeypatch) -> None:
    from verifier import __main__ as cli_mod
    from verifier.worker import VerifierError

    def boom(bundle, caller=None):  # noqa: ARG001
        raise VerifierError(
            "VerificationVerdict failed validation after retry: rationale "
            "must be at least 20 characters (ADR-018 sh:minLength); got 2"
        )

    monkeypatch.setattr(cli_mod, "run_verifier", boom)
    code, _, err = _run_cli(_bundle_blob(), monkeypatch)
    assert code == 1
    assert "rationale below minimum length" in err
