"""Worker SDK for emitting feedback artifacts (FT-031 / ADR-022)."""

from __future__ import annotations

import json
import sys
from typing import IO, Literal

from pydantic import BaseModel, Field, field_validator

FeedbackClass = Literal[
    "gap",
    "contradiction",
    "unimplementable",
    "scope-issue",
    "defect",
    "capability-request",
]

#: Sentinel prefix the harness scans for on a worker's stdout / stderr
#: to recognise an emitted feedback record. Keeps feedback records
#: lexically separable from a worker's normal ``WorkerResponse`` JSON.
FEEDBACK_RECORD_SENTINEL = "__DEC_FEEDBACK__"


class FeedbackEmission(BaseModel):
    """A single feedback emission produced by a worker (ADR-022).

    The wire shape is JSON, prefixed with :data:`FEEDBACK_RECORD_SENTINEL`
    on a dedicated line so the harness can parse it out of the worker's
    output stream.
    """

    feedback_class: FeedbackClass = Field(
        ..., description="ADR-023 controlled-vocabulary class tag.",
    )
    severity: Literal["low", "medium", "high"] = Field(
        default="medium",
        description="Severity hint ('low' / 'medium' / 'high').",
    )
    evidence: str = Field(
        ...,
        min_length=20,
        description="Free-form citation into the bundle (≥ 20 chars).",
    )
    recommendation: str | None = Field(
        default=None,
        description="Optional suggested fix the target role may consult.",
    )
    target_role_override: str | None = Field(
        default=None,
        description="Per-emission target role override (defaults to the class's "
        "default target per ADR-026).",
    )
    blocking: bool | None = Field(
        default=None,
        description="Per-emission blocking override (None = class default).",
    )
    disposition_rationale: str | None = Field(
        default=None,
        description="Rationale recorded when blocking differs from the class default.",
    )
    source_session: str = Field(
        default="",
        description="Optional session IRI; the harness populates this from the "
        "active session if the worker leaves it blank.",
    )

    @field_validator("evidence")
    @classmethod
    def _evidence_not_blank(cls, value: str) -> str:
        if not value.strip():
            raise ValueError("evidence must not be blank")
        return value

    def to_record(self) -> dict:
        """Serialise to the wire dict the harness reads from worker stdout."""
        return self.model_dump(exclude_none=True)


def emit_feedback(
    emission: FeedbackEmission,
    *,
    stream: IO[str] | None = None,
) -> str:
    """Write a single feedback record to ``stream`` (default: ``sys.stdout``).

    Returns the rendered record line (without the trailing newline) so
    callers can record it in session telemetry. The line format is:

        ``__DEC_FEEDBACK__ {json-payload}``

    The harness scans the worker's output for lines starting with the
    sentinel and parses the JSON payload that follows. The worker's
    normal ``WorkerResponse`` JSON is unaffected because it does not
    carry the sentinel.

    Per ADR-008 the worker never writes to the graph directly — this
    function only emits a record that the harness ingests; the harness
    constructs the actual ``dec:Feedback`` artifact via ``StreamWriter``.
    """
    target = stream if stream is not None else sys.stdout
    payload = json.dumps(emission.to_record(), separators=(",", ":"), sort_keys=True)
    line = f"{FEEDBACK_RECORD_SENTINEL} {payload}"
    target.write(line + "\n")
    target.flush()
    return line
