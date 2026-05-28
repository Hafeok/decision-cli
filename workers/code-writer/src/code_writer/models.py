"""Structured dispatch/response models for the code-writer worker.

Slice 1 contract (ADR-008): bundle in, structured artifact out.

* ``DispatchPayload`` — what the harness hands the worker (delivered over
  SSE in daemon mode, or read from stdin in one-shot mode).
* ``CodeChange``     — the typed artifact returned on success; this is
  what the harness writes into product-cli's graph.
* ``WorkerTelemetry`` — per-dispatch session telemetry (turn count,
  latency, tool-call history, errors).
* ``WorkerResponse`` — wrapper that carries either a successful
  ``CodeChange`` plus telemetry, or a structured error (ADR-008
  §Error handling).

All shapes are Pydantic models so the harness can validate the JSON
payload deterministically.
"""

from __future__ import annotations

from typing import Any, Literal

from pydantic import BaseModel, Field


class DispatchPayload(BaseModel):
    """Inputs the harness hands the worker for a single dispatch."""

    dispatch_id: str = Field(..., description="IRI of the dec:Dispatch artifact.")
    session_id: str = Field(..., description="IRI of the dec:Session artifact.")
    feature_id: str = Field(
        ..., description="Feature being implemented (e.g. 'FT-007')."
    )
    bundle_markdown: str = Field(
        ..., description="Serialised context bundle assembled by product-cli."
    )
    bundle_hash: str = Field(..., description="SHA-256 hex of bundle_markdown bytes.")
    workspace_path: str = Field(
        ..., description="Absolute path where the worker may write files."
    )
    model_id: str = Field(
        ..., description="Identifier of the model the worker should invoke."
    )
    max_turns: int = Field(default=8, ge=1, le=64)
    timeout_seconds: int = Field(default=300, ge=10, le=3600)
    allowed_tools: list[str] = Field(
        default_factory=lambda: ["Read", "Write", "Edit", "Glob", "Grep", "Bash"]
    )

    # pydantic v2 namespaces — `model_` is a reserved prefix on BaseModel
    # so we tell pydantic this is fine. We intentionally use `model_id`
    # to match the harness vocabulary (ADR-004 PROV-O `dec:modelVersion`).
    model_config = {"protected_namespaces": ()}


class FileWrite(BaseModel):
    """A single file written or edited during the dispatch."""

    path: str = Field(..., description="Workspace-relative path of the file.")
    summary: str = Field(default="", description="Diff summary or change description.")
    bytes_written: int = Field(default=0, ge=0)


class CodeChange(BaseModel):
    """The structured artifact produced by a successful dispatch (FT-013)."""

    iri: str = Field(..., description="IRI of the persisted CodeChange artifact.")
    feature_id: str = Field(..., description="Feature this CodeChange addresses.")
    session_id: str = Field(
        ..., description="Session IRI that produced this CodeChange (PROV-O)."
    )
    files: list[FileWrite] = Field(
        default_factory=list, description="Files written/edited during this run."
    )
    summary: str = Field(default="", description="Free-text summary from the model.")


class ToolCall(BaseModel):
    """One tool invocation observed in the Claude Code stream."""

    name: str
    arguments: dict[str, Any] = Field(default_factory=dict)
    result_status: Literal["ok", "error"] = "ok"


class WorkerTelemetry(BaseModel):
    """Per-dispatch session telemetry (ADR-008)."""

    turn_count: int = 0
    latency_seconds: float = 0.0
    tool_calls: list[ToolCall] = Field(default_factory=list)
    errors: list[str] = Field(default_factory=list)
    stdout_excerpt: str = ""
    stderr_excerpt: str = ""


class WorkerError(BaseModel):
    """Structured error report (ADR-008 §Error handling)."""

    category: Literal[
        "subscription_unavailable",
        "subprocess_failed",
        "unparseable_output",
        "workspace_violation",
        "timeout",
        "invalid_dispatch",
        "internal",
    ]
    message: str
    detail: str = ""
    retryable: bool = False


class WorkerResponse(BaseModel):
    """Wrapper for a worker run — either success (CodeChange) or error."""

    dispatch_id: str
    session_id: str
    status: Literal["ok", "error"]
    code_change: CodeChange | None = None
    telemetry: WorkerTelemetry = Field(default_factory=WorkerTelemetry)
    error: WorkerError | None = None
