"""TC-323: adr-quality verdict carries dec:judges (AdrProposal) + dec:against (gap + spec) per ADR-074."""

from __future__ import annotations

import json

from adr_quality.bundle import AdrQualityInput
from adr_quality.output import QualityVerdict
from adr_quality.worker import run_judge

from _helpers import build_bundle, extract_bundle_hash


def _mock_caller_polymorphism(system: str, user: str, model_id: str, max_tokens: int):
    """Mock caller returning an approved verdict with explicit judges + against IRIs.

    The verdict is approval-grade so attention stays on the polymorphism shape
    rather than the rubric flips (those are covered by TC-320 / TC-321 / TC-322).
    """
    bundle_hash = extract_bundle_hash(user)

    verdict = {
        "verdict": "approved",
        "rationale": (
            "Schema-conforming, gap-closing, scope-correct, domain-valid, and "
            "alternatives-noted. The proposal is fit for human acceptance."
        ),
        "judges": "urn:adr-proposal:FT-101",
        "against": [
            "urn:preflight-gap:adr:ADR-013",
            "urn:feature-spec:FT-101",
        ],
        "violates": [],
        "bundle_hash": bundle_hash,
    }
    return json.dumps(verdict), 100, 50


def _resolve_in_bundle(iri: str, bundle: AdrQualityInput) -> str:
    """Return the typed class of `iri` as inferred from its presence in the bundle.

    The bundle is the polymorphism oracle in the worker layer:
    * AdrProposal IRI recognised by `bundle.adr_proposal.iri`
    * PreflightGap IRI recognised by `bundle.preflight_gap_iri`
    * FeatureSpec IRI recognised by `bundle.resolved_feature_spec_iri`

    The harness graph-layer SHACL shape enforces the same on persistence
    (ADR-074); the worker only emits.
    """
    if iri == bundle.adr_proposal.iri:
        return "dec:AdrProposal"
    if iri == bundle.preflight_gap_iri:
        return "dec:PreflightGap"
    if iri == bundle.resolved_feature_spec_iri:
        return "dec:FeatureSpec"
    return "(unresolved)"


def test_judges_resolves_to_adr_proposal_against_to_gap_and_spec():
    """TC-323: judges + against carry the polymorphism contract under ADR-074."""
    bundle = build_bundle(
        bundle_hash="poly1234",
        proposal_iri="urn:adr-proposal:FT-101",
        feature_id="FT-101",
        feature_spec_iri="urn:feature-spec:FT-101",
        preflight_gap_iri="urn:preflight-gap:adr:ADR-013",
        proposal_kind="new",
        proposal_scope="cross-cutting",
    )

    result = run_judge(bundle, caller=_mock_caller_polymorphism)
    verdict = result.verdict

    # Worker exits with code 0 — pytest framework's own exit code reflects
    # this assertion passing. (exit-code observable)
    assert verdict.verdict == "approved"

    # The verdict's `judges` field is exactly one IRI; that IRI resolves
    # in the bundle to a node typed dec:AdrProposal. (graph observable)
    assert isinstance(verdict.judges, str)
    assert verdict.judges == "urn:adr-proposal:FT-101"
    assert _resolve_in_bundle(verdict.judges, bundle) == "dec:AdrProposal"

    # The verdict's `against` field contains exactly two IRIs — the
    # preflight_gap IRI and the feature_spec IRI. (graph observable)
    assert isinstance(verdict.against, list)
    assert len(verdict.against) == 2
    resolved_types = {_resolve_in_bundle(iri, bundle) for iri in verdict.against}
    assert "dec:PreflightGap" in resolved_types
    assert "dec:FeatureSpec" in resolved_types
    assert "urn:preflight-gap:adr:ADR-013" in verdict.against
    assert "urn:feature-spec:FT-101" in verdict.against

    # Neither judges nor against is empty — satisfies the minCount=1
    # constraints of the ADR-074 SHACL shape on QualityVerdict.
    assert verdict.judges
    assert verdict.against

    # The verdict serialises cleanly through the QualityVerdict pydantic
    # model without missing-field errors. (stdout observable)
    payload = json.loads(verdict.model_dump_json(exclude_none=True))
    reparsed = QualityVerdict.model_validate(payload)
    assert reparsed.judges == verdict.judges
    assert reparsed.against == verdict.against
    assert reparsed.bundle_hash == "poly1234"
