"""Deterministic stub runner used when ``CODE_WRITER_STUB=1``."""

from __future__ import annotations

import time
from dataclasses import dataclass
from pathlib import Path

from ._runner_common import STUB_ENV_VAR, _make_code_change_iri, _safe_join
from .models import (
    CodeChange,
    DispatchPayload,
    FileWrite,
    ToolCall,
    WorkerError,
    WorkerResponse,
    WorkerTelemetry,
)


@dataclass
class StubResult:
    """The canned write the stub runner emits."""

    rel_path: str
    contents: str
    summary: str


def _default_stub_result(payload: DispatchPayload) -> StubResult:
    """Deterministic stub artifact for tests."""
    feature = payload.feature_id.replace("/", "_")
    rel = f"stub-output/{feature}.md"
    body = (
        f"# Stub code-writer output for {payload.feature_id}\n\n"
        f"This file was produced by the code-writer worker in stub mode\n"
        f"({STUB_ENV_VAR}=1) so the end-to-end harness flow can be exercised\n"
        f"without invoking `claude -p` against a real subscription session.\n"
        f"\n"
        f"Dispatch: {payload.dispatch_id}\n"
        f"Session:  {payload.session_id}\n"
        f"Bundle SHA-256: {payload.bundle_hash}\n"
    )
    summary = f"stub: wrote {rel} ({len(body)} bytes) for {payload.feature_id}"
    return StubResult(rel_path=rel, contents=body, summary=summary)


def run_stub(payload: DispatchPayload) -> WorkerResponse:
    """Stub runner — no subprocess, deterministic output.

    This is the path TC-008 and TC-013 exercise.
    """
    started = time.monotonic()
    workspace = Path(payload.workspace_path)
    workspace.mkdir(parents=True, exist_ok=True)
    stub = _default_stub_result(payload)
    try:
        target = _safe_join(workspace, stub.rel_path)
    except ValueError as exc:
        return WorkerResponse(
            dispatch_id=payload.dispatch_id,
            session_id=payload.session_id,
            status="error",
            error=WorkerError(
                category="workspace_violation",
                message=str(exc),
                retryable=False,
            ),
        )
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text(stub.contents, encoding="utf-8")

    file_write = FileWrite(
        path=stub.rel_path,
        summary=stub.summary,
        bytes_written=len(stub.contents.encode("utf-8")),
    )
    code_change = CodeChange(
        iri=_make_code_change_iri(payload.dispatch_id),
        feature_id=payload.feature_id,
        session_id=payload.session_id,
        files=[file_write],
        summary=stub.summary,
    )
    telemetry = WorkerTelemetry(
        turn_count=1,
        latency_seconds=time.monotonic() - started,
        tool_calls=[
            ToolCall(
                name="Write",
                arguments={"file_path": stub.rel_path},
                result_status="ok",
            )
        ],
        stdout_excerpt="stub-mode",
    )
    return WorkerResponse(
        dispatch_id=payload.dispatch_id,
        session_id=payload.session_id,
        status="ok",
        code_change=code_change,
        telemetry=telemetry,
    )
