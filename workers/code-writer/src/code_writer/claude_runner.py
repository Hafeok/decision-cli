"""Invoke Claude Code in headless mode (``claude -p``) and observe writes.

This module implements ADR-008 §Behaviour step 3-5 for FT-013:

1. Spawn ``claude -p`` with ``--output-format stream-json`` so tool-call
   and result events surface deterministically.
2. Pipe the bundle markdown to stdin as the prompt.
3. Stream stdout, parsing JSONL events into per-turn telemetry and a
   final result block.
4. Build a :class:`CodeChange` from the observed ``Edit`` / ``Write``
   tool calls and the model's final summary.

A "stub mode" (controlled by the ``CODE_WRITER_STUB=1`` env var or
``--stub`` flag) bypasses the ``claude`` subprocess entirely. The stub
writes a deterministic marker file inside the workspace and returns a
synthetic :class:`CodeChange` so the harness end-to-end flow can be
exercised in CI without depending on a Claude subscription. This is the
single switch every TC needs.
"""

from __future__ import annotations

import json
import os
import shutil
import subprocess
import time
from dataclasses import dataclass
from pathlib import Path
from typing import Any

from .models import (
    CodeChange,
    DispatchPayload,
    FileWrite,
    ToolCall,
    WorkerError,
    WorkerResponse,
    WorkerTelemetry,
)

STUB_ENV_VAR = "CODE_WRITER_STUB"


def _is_stub_mode() -> bool:
    return os.environ.get(STUB_ENV_VAR, "").strip() not in {"", "0", "false", "no"}


def _safe_join(workspace: Path, rel: str) -> Path:
    """Resolve ``rel`` against ``workspace`` and reject path traversal.

    ADR-008 invariant: files written must be confined to the workspace.
    Any attempt to escape via ``..`` or absolute paths is treated as
    ``workspace_violation``.
    """
    candidate = (workspace / rel).resolve()
    workspace_resolved = workspace.resolve()
    if not str(candidate).startswith(str(workspace_resolved)):
        raise ValueError(f"path {rel!r} escapes workspace {workspace!s}")
    return candidate


def _make_code_change_iri(dispatch_id: str) -> str:
    """Mint a deterministic CodeChange IRI tied to the dispatch."""
    # We tie the CodeChange IRI to the dispatch IRI so the cross-graph
    # PROV-O linkage (TC-013) is reproducible. If `dispatch_id` is itself
    # an IRI, derive a sibling IRI under the same namespace.
    if "://" in dispatch_id:
        # Replace the trailing path segment with `code-change/<tail>`.
        head, _, tail = dispatch_id.rpartition("/")
        return f"{head}/code-change/{tail or 'unnamed'}"
    return f"urn:dec:code-change:{dispatch_id}"


@dataclass
class StubResult:
    """The canned write the stub runner emits."""

    rel_path: str
    contents: str
    summary: str


def _default_stub_result(payload: DispatchPayload) -> StubResult:
    """Deterministic stub artifact for tests.

    The file is rooted at ``stub-output/<feature>.md`` inside the
    workspace so test fixtures can predict the location.
    """
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
    summary = (
        f"stub: wrote {rel} ({len(body)} bytes) for {payload.feature_id}"
    )
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


def _claude_on_path() -> str | None:
    return shutil.which("claude")


def _parse_stream_json(blob: str) -> tuple[list[ToolCall], list[FileWrite], str, list[str]]:
    """Parse Claude Code's ``--output-format stream-json`` output.

    Returns ``(tool_calls, file_writes, final_summary, errors)``.

    The stream is JSONL; each line is an event. We are tolerant about the
    exact schema (Claude Code's stream format is allowed to evolve) — we
    look for ``Edit`` / ``Write`` tool invocations to populate file
    writes, and the final ``result`` event for the summary.
    """
    tool_calls: list[ToolCall] = []
    file_writes: list[FileWrite] = []
    final_summary = ""
    errors: list[str] = []

    for raw in blob.splitlines():
        line = raw.strip()
        if not line:
            continue
        try:
            event = json.loads(line)
        except json.JSONDecodeError as exc:
            errors.append(f"unparseable stream-json line: {exc}")
            continue
        if not isinstance(event, dict):
            continue
        event_type = event.get("type", "")
        if event_type in {"tool_use", "tool_call"}:
            name = str(event.get("name", "")) or str(event.get("tool", ""))
            args: dict[str, Any] = event.get("input") or event.get("arguments") or {}
            tool_calls.append(
                ToolCall(name=name, arguments=args if isinstance(args, dict) else {})
            )
            if name in {"Write", "Edit"} and isinstance(args, dict):
                path = str(args.get("file_path") or args.get("path") or "")
                if path:
                    file_writes.append(
                        FileWrite(path=path, summary=f"{name} via claude -p")
                    )
        elif event_type == "result":
            final_summary = str(event.get("result") or event.get("summary") or "")
        elif event_type == "error":
            errors.append(str(event.get("message") or event))

    return tool_calls, file_writes, final_summary, errors


def run_claude(payload: DispatchPayload) -> WorkerResponse:
    """Real ``claude -p`` subprocess runner (ADR-008 §Behaviour 3-5).

    The implementation is intentionally conservative: we shell out to
    ``claude`` with ``--output-format stream-json``, feed the bundle on
    stdin, and parse the JSONL output. Failures are translated into
    structured ``WorkerError`` values so the harness can decide retry
    semantics.
    """
    binary = _claude_on_path()
    if binary is None:
        return WorkerResponse(
            dispatch_id=payload.dispatch_id,
            session_id=payload.session_id,
            status="error",
            error=WorkerError(
                category="subscription_unavailable",
                message="`claude` binary not found on $PATH",
                detail=(
                    "Install Claude Code and run `claude login` once on this host. "
                    f"Alternatively, run the worker with {STUB_ENV_VAR}=1 to use "
                    "the deterministic stub runner."
                ),
                retryable=False,
            ),
        )

    workspace = Path(payload.workspace_path)
    workspace.mkdir(parents=True, exist_ok=True)
    args = [
        binary,
        "-p",
        "--output-format",
        "stream-json",
        "--max-turns",
        str(payload.max_turns),
    ]
    if payload.allowed_tools:
        args.extend(["--allowedTools", ",".join(payload.allowed_tools)])

    started = time.monotonic()
    try:
        completed = subprocess.run(
            args,
            input=payload.bundle_markdown,
            capture_output=True,
            text=True,
            timeout=payload.timeout_seconds,
            cwd=str(workspace),
            check=False,
        )
    except subprocess.TimeoutExpired as exc:
        return WorkerResponse(
            dispatch_id=payload.dispatch_id,
            session_id=payload.session_id,
            status="error",
            error=WorkerError(
                category="timeout",
                message=f"`claude -p` exceeded {payload.timeout_seconds}s",
                detail=str(exc),
                retryable=True,
            ),
        )

    latency = time.monotonic() - started
    if completed.returncode != 0:
        return WorkerResponse(
            dispatch_id=payload.dispatch_id,
            session_id=payload.session_id,
            status="error",
            error=WorkerError(
                category="subprocess_failed",
                message=f"`claude -p` exited with status {completed.returncode}",
                detail=completed.stderr[-2000:],
                retryable=False,
            ),
            telemetry=WorkerTelemetry(
                latency_seconds=latency,
                stdout_excerpt=completed.stdout[-2000:],
                stderr_excerpt=completed.stderr[-2000:],
                errors=[completed.stderr[-2000:]] if completed.stderr else [],
            ),
        )

    tool_calls, file_writes, final_summary, parse_errors = _parse_stream_json(
        completed.stdout
    )

    # Workspace confinement enforcement (ADR-008 invariant).
    confined: list[FileWrite] = []
    for fw in file_writes:
        try:
            _safe_join(workspace, fw.path)
        except ValueError as exc:
            return WorkerResponse(
                dispatch_id=payload.dispatch_id,
                session_id=payload.session_id,
                status="error",
                error=WorkerError(
                    category="workspace_violation",
                    message=str(exc),
                    detail=f"file path {fw.path!r} escapes {workspace!s}",
                    retryable=False,
                ),
                telemetry=WorkerTelemetry(
                    latency_seconds=latency,
                    stdout_excerpt=completed.stdout[-2000:],
                ),
            )
        confined.append(fw)

    code_change = CodeChange(
        iri=_make_code_change_iri(payload.dispatch_id),
        feature_id=payload.feature_id,
        session_id=payload.session_id,
        files=confined,
        summary=final_summary,
    )
    telemetry = WorkerTelemetry(
        turn_count=len(
            [tc for tc in tool_calls if tc.name in {"Write", "Edit", "Bash"}]
        )
        or 1,
        latency_seconds=latency,
        tool_calls=tool_calls,
        errors=parse_errors,
        stdout_excerpt=completed.stdout[-2000:],
        stderr_excerpt=completed.stderr[-2000:],
    )
    return WorkerResponse(
        dispatch_id=payload.dispatch_id,
        session_id=payload.session_id,
        status="ok",
        code_change=code_change,
        telemetry=telemetry,
    )


def run_dispatch(payload: DispatchPayload, *, force_stub: bool | None = None) -> WorkerResponse:
    """Single dispatch entry point — picks stub vs. real runner.

    ``force_stub`` overrides the env var (used by the one-shot CLI's
    ``--stub`` flag). When ``None`` (the default), the env var
    ``CODE_WRITER_STUB`` is consulted.
    """
    use_stub = _is_stub_mode() if force_stub is None else force_stub
    if use_stub:
        return run_stub(payload)
    return run_claude(payload)
