"""Serialises a validated VerificationVerdict to the harness JSON shape."""

from __future__ import annotations

from typing import Literal

from pydantic import BaseModel, ConfigDict, Field, field_validator, model_validator

# ADR-018 §SHACL shape: rationale ≥ 20 chars.
MIN_RATIONALE_LENGTH = 20

Verdict = Literal["approved", "rejected", "amendment-required"]


class VerificationVerdict(BaseModel):
    """Single artifact returned by the verifier worker (ADR-018).

    The shape mirrors the SHACL `dec:VerificationVerdictShape`:

    * exactly one of three verdict values,
    * rationale of at least 20 characters,
    * ≥ 1 `dec:violates` reference when `verdict ≠ approved`,
    * `amendmentGuidance` required iff `verdict == amendment-required`.

    Pydantic enforces the field-level constraints; the conditional
    constraints are checked in `model_validator`.
    """

    # Refuse unknown / extra fields — ADR-008 worker contract demands a
    # strict shape so the harness can validate the JSON deterministically.
    model_config = ConfigDict(extra="forbid")

    verdict: Verdict = Field(..., description="One of approved/rejected/amendment-required.")
    rationale: str = Field(
        ...,
        description="Free-form prose, ≥ 20 chars (ADR-018 sh:minLength).",
    )
    violates: list[str] = Field(
        default_factory=list,
        description="TC-XXX / ADR-XXX references the verdict cites. Required "
        "when verdict ≠ approved.",
    )
    amendment_guidance: str | None = Field(
        default=None,
        description="Required iff verdict == amendment-required.",
    )

    @field_validator("rationale")
    @classmethod
    def _rationale_min_length(cls, value: str) -> str:
        stripped = value.strip()
        if len(stripped) < MIN_RATIONALE_LENGTH:
            raise ValueError(
                f"rationale must be at least {MIN_RATIONALE_LENGTH} characters "
                f"(ADR-018 sh:minLength); got {len(stripped)}"
            )
        return value

    @model_validator(mode="after")
    def _conditional_constraints(self) -> "VerificationVerdict":
        if self.verdict in ("rejected", "amendment-required") and not self.violates:
            raise ValueError(
                f"{self.verdict} verdict must cite at least one TC or ADR via "
                f"'violates' (ADR-018 SHACL sh:sparql constraint)"
            )
        if self.verdict == "amendment-required" and not (
            self.amendment_guidance and self.amendment_guidance.strip()
        ):
            raise ValueError(
                "amendment-required verdict must carry non-empty "
                "'amendment_guidance' (ADR-018 sh:sparql constraint)"
            )
        if self.verdict == "approved" and self.amendment_guidance:
            raise ValueError(
                "approved verdict must not carry amendment_guidance "
                "(amendment_guidance is for amendment-required only)"
            )
        return self
