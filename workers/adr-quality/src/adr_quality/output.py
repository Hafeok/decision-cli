"""Output artifact: QualityVerdict (ADR-074)."""

from __future__ import annotations

from pydantic import BaseModel, Field, field_validator


class QualityVerdict(BaseModel):
    """An adr-quality verdict judging an AdrProposal's fitness (ADR-074, FT-133).

    Structurally identical to VerificationVerdict (ADR-018) except for:
    - `judges` (IRI of the AdrProposal under judgment) instead of `verifies`
    - `against` (list of source-of-truth IRIs — the preflight_gap and
      feature_spec) instead of a single target

    The three-verdict vocabulary ('approved', 'rejected', 'amendment-required')
    is inherited from ADR-018. The SHACL constraints (rationale >= 20 chars,
    rejected/amendment-required must cite violates, amendment-required must
    carry amendment_guidance) are enforced at the graph layer; this Pydantic
    model provides the worker-side validation.
    """

    verdict: str = Field(
        ...,
        description="One of: 'approved', 'rejected', 'amendment-required'.",
    )
    rationale: str = Field(
        ...,
        min_length=20,
        description="Substantive explanation for the verdict (>= 20 chars per ADR-018).",
    )
    judges: str = Field(
        ...,
        description="IRI of the AdrProposal artifact under judgment (ADR-074 polymorphism).",
    )
    against: list[str] = Field(
        ...,
        min_length=1,
        description=(
            "Source-of-truth IRIs the artifact is judged against "
            "(preflight_gap IRI + feature_spec IRI for ADR quality)."
        ),
    )
    violates: list[str] = Field(
        default_factory=list,
        description="IRIs or identifiers of violated references (required for rejected/amendment-required).",
    )
    amendment_guidance: str | None = Field(
        None,
        description="Concrete guidance for amendment-required verdicts (required when verdict='amendment-required').",
    )
    bundle_hash: str = Field(
        ...,
        min_length=8,
        description="Echoed bundle_hash from the input (provenance check per ADR-073).",
    )

    @field_validator("verdict")
    @classmethod
    def validate_verdict(cls, v: str) -> str:
        """Enforce the three-verdict vocabulary."""
        if v not in {"approved", "rejected", "amendment-required"}:
            raise ValueError(
                f"verdict must be one of 'approved', 'rejected', 'amendment-required'; got '{v}'"
            )
        return v

    @field_validator("rationale")
    @classmethod
    def validate_rationale(cls, v: str) -> str:
        """Enforce minimum length."""
        if len(v) < 20:
            raise ValueError(f"rationale must be at least 20 characters; got {len(v)}")
        return v

    def model_post_init(self, __context) -> None:
        """Cross-field validation: rejected/amendment-required MUST cite violates."""
        if self.verdict in {"rejected", "amendment-required"} and not self.violates:
            raise ValueError(
                f"verdict='{self.verdict}' requires at least one violated reference via 'violates'"
            )
        if self.verdict == "amendment-required" and not self.amendment_guidance:
            raise ValueError(
                "verdict='amendment-required' requires 'amendment_guidance' to be populated"
            )
