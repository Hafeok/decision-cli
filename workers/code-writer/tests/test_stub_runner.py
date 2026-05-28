"""Unit tests for the stub runner path (ADR-008 contract surface).

The stub runner is the test-mode interface every TC drives, so its
behaviour is part of the public contract: it MUST write the declared
file, MUST honour workspace confinement, and MUST emit a structured
``WorkerResponse`` with telemetry.
"""

from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path

import pytest

from code_writer.claude_runner import run_dispatch, run_stub
from code_writer.models import DispatchPayload


def _payload(workspace: Path, feature: str = "FT-013") -> DispatchPayload:
    return DispatchPayload(
        dispatch_id=f"urn:dec:dispatch:{feature}",
        session_id=f"urn:dec:session:{feature}",
        feature_id=feature,
        bundle_markdown=f"# {feature}\n\nstub bundle\n",
        bundle_hash="a" * 64,
        workspace_path=str(workspace),
        model_id="claude-sonnet-4-5",
    )


def test_stub_writes_file_inside_workspace(tmp_path: Path) -> None:
    payload = _payload(tmp_path)
    response = run_stub(payload)
    assert response.status == "ok"
    assert response.code_change is not None
    assert len(response.code_change.files) == 1
    written = tmp_path / response.code_change.files[0].path
    assert written.exists()
    assert payload.feature_id in written.read_text()


def test_stub_telemetry_is_populated(tmp_path: Path) -> None:
    payload = _payload(tmp_path)
    response = run_stub(payload)
    assert response.telemetry.turn_count == 1
    assert response.telemetry.latency_seconds >= 0.0
    assert any(tc.name == "Write" for tc in response.telemetry.tool_calls)


def test_force_stub_via_run_dispatch(tmp_path: Path) -> None:
    payload = _payload(tmp_path)
    response = run_dispatch(payload, force_stub=True)
    assert response.status == "ok"
    assert response.code_change is not None
    assert response.code_change.feature_id == payload.feature_id
    assert response.code_change.session_id == payload.session_id


def test_run_dispatch_no_claude_returns_structured_error(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    # Force real mode and pretend `claude` is not on $PATH.
    monkeypatch.delenv("CODE_WRITER_STUB", raising=False)
    monkeypatch.setattr(
        "code_writer.claude_runner._claude_on_path",
        lambda: None,
    )
    response = run_dispatch(_payload(tmp_path), force_stub=False)
    assert response.status == "error"
    assert response.error is not None
    assert response.error.category == "subscription_unavailable"


def test_code_change_iri_is_deterministic(tmp_path: Path) -> None:
    payload = _payload(tmp_path)
    response_a = run_stub(payload)
    response_b = run_stub(payload)
    assert response_a.code_change.iri == response_b.code_change.iri  # type: ignore[union-attr]


def test_cli_run_once_round_trip(tmp_path: Path) -> None:
    """End-to-end: shell out to `python -m code_writer.main run-once --stub`."""
    payload = _payload(tmp_path)
    proc = subprocess.run(
        [sys.executable, "-m", "code_writer.main", "run-once", "--stub"],
        input=payload.model_dump_json(),
        capture_output=True,
        text=True,
        check=False,
    )
    assert proc.returncode == 0, proc.stderr
    decoded = json.loads(proc.stdout.strip().splitlines()[-1])
    assert decoded["status"] == "ok"
    assert decoded["code_change"]["feature_id"] == payload.feature_id


def test_cli_run_once_rejects_empty_stdin(tmp_path: Path) -> None:
    proc = subprocess.run(
        [sys.executable, "-m", "code_writer.main", "run-once", "--stub"],
        input="",
        capture_output=True,
        text=True,
        check=False,
    )
    assert proc.returncode == 2
    payload = json.loads(proc.stdout.strip())
    assert payload["status"] == "error"
    assert payload["error"]["category"] == "invalid_dispatch"
