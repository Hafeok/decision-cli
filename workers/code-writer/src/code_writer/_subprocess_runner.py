"""Real ``claude -p`` subprocess runner (ADR-008 §Behaviour 3-5)."""

from __future__ import annotations

import json
import os
import subprocess
import tempfile
import time
from pathlib import Path
from typing import Any

from . import claude_runner as _entry  # late import for test-mock hook
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


def _parse_stream_json(
    blob: str,
) -> tuple[list[ToolCall], list[FileWrite], str, list[str]]:
    """Parse Claude Code's ``--output-format stream-json`` output.

    Returns ``(tool_calls, file_writes, final_summary, errors)``.
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


def _scrape_terminal_error(stdout: str) -> str:
    """Pull the final stream-json result event out of stdout."""
    for raw in reversed(stdout.splitlines()):
        line = raw.strip()
        if not line:
            continue
        try:
            event = json.loads(line)
        except json.JSONDecodeError:
            continue
        if isinstance(event, dict) and event.get("type") == "result":
            pieces = [
                f"subtype={event.get('subtype')}",
                f"terminal_reason={event.get('terminal_reason')}",
                f"num_turns={event.get('num_turns')}",
            ]
            if event.get("errors"):
                pieces.append(f"errors={event['errors']}")
            if event.get("result"):
                pieces.append(f"result={str(event['result'])[:500]}")
            return "; ".join(pieces)
    return stdout[-2000:]


def run_claude(payload: DispatchPayload) -> WorkerResponse:
    """Real ``claude -p`` subprocess runner (ADR-008 §Behaviour 3-5)."""
    binary = _entry._claude_on_path()
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

    prompt_fd, prompt_path = tempfile.mkstemp(
        prefix=f"dec-bundle-{payload.feature_id}-",
        suffix=".md",
    )
    with os.fdopen(prompt_fd, "w", encoding="utf-8") as fh:
        fh.write(payload.bundle_markdown)
    user_message = (
        f"Implement feature {payload.feature_id} described in the system "
        "prompt. Follow all constraints and run `product verify` when done."
    )
    args = [
        binary,
        "-p",
        "--dangerously-skip-permissions",
        "--system-prompt-file",
        prompt_path,
        "--output-format",
        "stream-json",
        "--verbose",
        user_message,
    ]

    started = time.monotonic()
    try:
        try:
            completed = subprocess.run(
                args,
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
            detail = completed.stderr.strip()
            if not detail:
                detail = _scrape_terminal_error(completed.stdout)
            return WorkerResponse(
                dispatch_id=payload.dispatch_id,
                session_id=payload.session_id,
                status="error",
                error=WorkerError(
                    category="subprocess_failed",
                    message=f"`claude -p` exited with status {completed.returncode}",
                    detail=detail,
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
    finally:
        try:
            os.unlink(prompt_path)
        except OSError:
            pass
