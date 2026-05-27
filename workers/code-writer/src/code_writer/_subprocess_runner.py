"""Real ``claude -p`` subprocess runner (ADR-008 §Behaviour 3-5)."""

from __future__ import annotations

import json
import os
import subprocess
import tempfile
import time
from pathlib import Path
from typing import Any

import re

from . import claude_runner as _entry  # late import for test-mock hook
from ._runner_common import STUB_ENV_VAR, _make_code_change_iri, _safe_join
from .env_routing import EndpointConfigError, claude_env_for
from .models import (
    CodeChange, DefectFeedbackRecord, DispatchPayload, FileWrite, ToolCall,
    WorkerError, WorkerResponse, WorkerTelemetry,
)


# FT-108: agent must end its final result with a marker-delimited JSON
# block whose `iris` field lists every feedback IRI from the bundle's
# `defect_feedback` array that this code change addresses. The Rust
# accept path uses this list to transition each cited feedback through
# the ADR-024 lifecycle to `addressed`.
ADDRESSED_FEEDBACK_BEGIN = "<<DEC_ADDRESSED_FEEDBACK>>"
ADDRESSED_FEEDBACK_END = "<<END_DEC_ADDRESSED_FEEDBACK>>"


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
            tool_calls.append(ToolCall(name=name, arguments=args if isinstance(args, dict) else {}))
            if name in {"Write", "Edit"} and isinstance(args, dict):
                path = str(args.get("file_path") or args.get("path") or "")
                if path:
                    file_writes.append(FileWrite(path=path, summary=f"{name} via claude -p"))
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


def _endpoint_config_response(
    payload: DispatchPayload, err: EndpointConfigError
) -> WorkerResponse:
    """Response returned when endpoint env-overlay construction fails (FT-066)."""
    return WorkerResponse(
        dispatch_id=payload.dispatch_id,
        session_id=payload.session_id,
        status="error",
        error=WorkerError(
            category="endpoint_config",
            message=err.message,
            detail=f"endpoint={payload.endpoint!r} sub_category={err.category!r}",
            retryable=False,
        ),
    )


def _missing_binary_response(payload: DispatchPayload) -> WorkerResponse:
    """Response returned when `claude` is not on $PATH."""
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


def _write_bundle_prompt(payload: DispatchPayload) -> str:
    """Persist the bundle as a temp `.md` file. Returns the file path.

    When the bundle carries defect feedback (FT-108), the prompt is
    augmented with a `## Runtime defect feedback` section + an explicit
    citation-block requirement so the agent's response can be parsed
    server-side for `addressed_feedback_iris`. Without this, the FT-108
    server-side guard rejects the dispatch with `WorkerIgnoredFeedback`.
    """
    prompt_body = payload.bundle_markdown
    if payload.defect_feedback:
        prompt_body = f"{prompt_body}\n\n{_render_defect_feedback_section(payload.defect_feedback)}"
    prompt_fd, prompt_path = tempfile.mkstemp(
        prefix=f"dec-bundle-{payload.feature_id}-",
        suffix=".md",
    )
    with os.fdopen(prompt_fd, "w", encoding="utf-8") as fh:
        fh.write(prompt_body)
    return prompt_path


def _render_defect_feedback_section(records: list[DefectFeedbackRecord]) -> str:
    """Render the FT-108 defect-feedback section for the agent prompt.

    Includes the IRIs verbatim so the agent can copy them into the
    citation block, plus an explicit terminator showing the exact
    output format the server-side extractor expects."""
    lines = [
        "## Runtime defect feedback (FT-108)",
        "",
        "Prior verification runs found the following defects against tests this feature owns. ",
        "Your code change MUST fix the underlying issues — read each entry's `evidence` for the runner diagnostic.",
        "",
    ]
    for r in records:
        lines.append(f"### {r.feedback_iri}")
        if r.source_tc:
            lines.append(f"- source TC: `{r.source_tc}`")
        lines.append(f"- severity: {r.severity}")
        lines.append(f"- evidence: {r.evidence.strip() or '(empty)'}")
        lines.append("")
    iris_json = json.dumps([r.feedback_iri for r in records], indent=2)
    lines.extend([
        "### REQUIRED — citation block",
        "",
        "After writing the code changes, your **final assistant message** MUST end with",
        "a marker-delimited JSON block listing every feedback IRI you actually addressed.",
        "The orchestrator parses this exactly; missing or malformed → dispatch is rejected.",
        "",
        "Format (substitute the IRIs YOU addressed — drop any you couldn't fix, but cite at least one):",
        "",
        "```",
        ADDRESSED_FEEDBACK_BEGIN,
        '{',
        f'  "iris": {_indent_json(iris_json, 2)}',
        '}',
        ADDRESSED_FEEDBACK_END,
        "```",
        "",
        "Use the EXACT marker strings above (no whitespace variation, no extra text inside the markers).",
    ])
    return "\n".join(lines)


def _indent_json(blob: str, indent_spaces: int) -> str:
    """Indent every line of a JSON blob past the first by `indent_spaces`."""
    pad = " " * indent_spaces
    lines = blob.splitlines()
    if not lines:
        return blob
    return lines[0] + "\n" + "\n".join(pad + line for line in lines[1:])


def _extract_addressed_feedback(blob: str) -> list[str]:
    """Pull `addressed_feedback_iris` out of the agent's final-summary text.

    Locates the most-recent `<<DEC_ADDRESSED_FEEDBACK>>...<<END>>` block
    in `blob`, parses its JSON body, and returns the `iris` array. Any
    parse failure / missing block → empty list (the Rust accept path
    surfaces `WorkerIgnoredFeedback` for the operator).
    """
    if not blob:
        return []
    pattern = re.compile(
        r"<<DEC_ADDRESSED_FEEDBACK>>\s*(\{.*?\})\s*<<END_DEC_ADDRESSED_FEEDBACK>>",
        re.DOTALL,
    )
    matches = list(pattern.finditer(blob))
    if not matches:
        return []
    raw = matches[-1].group(1)
    try:
        payload = json.loads(raw)
    except json.JSONDecodeError:
        return []
    iris = payload.get("iris") if isinstance(payload, dict) else None
    if not isinstance(iris, list):
        return []
    return [str(i) for i in iris if isinstance(i, str)]


def _build_claude_args(binary: str, prompt_path: str, payload: DispatchPayload) -> list[str]:
    """Compose the argv list for `claude -p` with stream-json output."""
    user_message = (
        f"Implement feature {payload.feature_id} described in the system "
        "prompt. Follow all constraints and run `product verify` when done."
    )
    return [
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


def _timeout_response(payload: DispatchPayload, exc: subprocess.TimeoutExpired) -> WorkerResponse:
    """Response returned when `claude -p` exceeded its timeout budget."""
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


def _nonzero_exit_response(
    payload: DispatchPayload,
    completed: subprocess.CompletedProcess[str],
    latency: float,
) -> WorkerResponse:
    """Response built from a non-zero `claude -p` exit code."""
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


def _workspace_violation_response(
    payload: DispatchPayload,
    workspace: Path,
    file_path: str,
    exc: ValueError,
    completed: subprocess.CompletedProcess[str],
    latency: float,
) -> WorkerResponse:
    """Response returned when the model tried to touch a file outside the workspace."""
    return WorkerResponse(
        dispatch_id=payload.dispatch_id,
        session_id=payload.session_id,
        status="error",
        error=WorkerError(
            category="workspace_violation",
            message=str(exc),
            detail=f"file path {file_path!r} escapes {workspace!s}",
            retryable=False,
        ),
        telemetry=WorkerTelemetry(
            latency_seconds=latency,
            stdout_excerpt=completed.stdout[-2000:],
        ),
    )


def _confine_writes(
    workspace: Path,
    file_writes: list[FileWrite],
) -> tuple[list[FileWrite], FileWrite | None, ValueError | None]:
    """Filter `file_writes` to those inside `workspace`.

    Returns the confined list, plus the first offender (if any) with its error.
    """
    confined: list[FileWrite] = []
    for fw in file_writes:
        try:
            _safe_join(workspace, fw.path)
        except ValueError as exc:
            return confined, fw, exc
        confined.append(fw)
    return confined, None, None


def _build_success_response(
    payload: DispatchPayload,
    confined: list[FileWrite],
    tool_calls: list[ToolCall],
    parse_errors: list[str],
    final_summary: str,
    completed: subprocess.CompletedProcess[str],
    latency: float,
) -> WorkerResponse:
    """Assemble the `status="ok"` response from parsed stream-json output."""
    # FT-108: parse the agent's citation block when the bundle asked for
    # one. The Rust accept path's WorkerIgnoredFeedback guard catches an
    # empty list when the bundle had defect feedback, so we don't have
    # to enforce non-emptiness here — surface what the model produced.
    addressed = _extract_addressed_feedback(final_summary) if payload.defect_feedback else []
    code_change = CodeChange(
        iri=_make_code_change_iri(payload.dispatch_id),
        feature_id=payload.feature_id,
        session_id=payload.session_id,
        files=confined,
        summary=final_summary,
        addressed_feedback_iris=addressed,
    )
    telemetry = WorkerTelemetry(
        turn_count=len([tc for tc in tool_calls if tc.name in {"Write", "Edit", "Bash"}]) or 1,
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


def run_claude(payload: DispatchPayload) -> WorkerResponse:
    """Real ``claude -p`` subprocess runner (ADR-008 §Behaviour 3-5)."""
    binary = _entry._claude_on_path()
    if binary is None:
        return _missing_binary_response(payload)

    # FT-066 / ADR-033 — translate the resolved capability's endpoint
    # into the right ANTHROPIC_* env vars BEFORE spawning. A missing
    # SCW_SECRET_KEY surfaces as a structured WorkerError pre-spawn so
    # the operator sees a clear failure instead of an upstream 401.
    try:
        spawn_env = claude_env_for(payload)
    except EndpointConfigError as exc:
        return _endpoint_config_response(payload, exc)

    workspace = Path(payload.workspace_path)
    workspace.mkdir(parents=True, exist_ok=True)
    prompt_path = _write_bundle_prompt(payload)
    args = _build_claude_args(binary, prompt_path, payload)

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
                env=spawn_env,
            )
        except subprocess.TimeoutExpired as exc:
            return _timeout_response(payload, exc)

        latency = time.monotonic() - started
        if completed.returncode != 0:
            return _nonzero_exit_response(payload, completed, latency)

        tool_calls, file_writes, final_summary, parse_errors = _parse_stream_json(completed.stdout)
        confined, offender, exc = _confine_writes(workspace, file_writes)
        if offender is not None and exc is not None:
            return _workspace_violation_response(
                payload, workspace, offender.path, exc, completed, latency
            )
        return _build_success_response(
            payload, confined, tool_calls, parse_errors, final_summary, completed, latency
        )
    finally:
        try:
            os.unlink(prompt_path)
        except OSError:
            pass
