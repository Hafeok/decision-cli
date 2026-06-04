"""TC-319: spec-quality verdict carries dec:judges (SpecProposal) + dec:against (request) per ADR-074."""

from __future__ import annotations

import json

from spec_quality.bundle import SpecQualityInput
from spec_quality.output import QualityVerdict
from spec_quality.worker import run_judge

from _helpers import build_bundle, extract_bundle_hash


def _mock_caller_polymorphism(system: str, user: str, model_id: str, max_tokens: int):
    """Mock caller that returns an approved verdict with explicit judges + against IRIs.

    The verdict is approval-grade so attention stays on the polymorphism shape
    rather than the rubric flips (those are covered by TC-316 / TC-317 / TC-318).
    """
    bundle_hash = extract_bundle_hash(user)

    verdict = {
        "verdict": "approved",
        "rationale": (
            "Schema-conforming, request-faithful, bounded, non-colliding, and "
            "linkage-sound. The proposal is fit for human acceptance."
        ),
        "judges": "urn:spec-proposal:REQ-007",
        "against": ["urn:request:REQ-007"],
        "violates": [],
        "bundle_hash": bundle_hash,
    }
    return json.dumps(verdict), 100, 50


def _resolve_in_bundle(iri: str, bundle: SpecQualityInput) -> str:
    """Return the typed class of `iri` as inferred from its presence in the bundle.

    The bundle is the polymorphism oracle in the worker layer: a SpecProposal IRI
    is recognised by `bundle.spec_proposal.iri`, a Request IRI by the URN's
    prefix mapping to `bundle.request.id`. The harness graph-layer SHACL shape
    enforces the same on persistence (ADR-074); the worker only emits.
    """
    if iri == bundle.spec_proposal.iri:
        return "dec:SpecProposal"
    if iri.endswith(bundle.request.id):
        return "dec:Request"
    return "(unresolved)"


def test_judges_resolves_to_spec_proposal_against_to_request():
    """TC-319: judges + against carry the polymorphism contract under ADR-074."""
    bundle = build_bundle(bundle_hash="poly1234", spec_iri="urn:spec-proposal:REQ-007")

    result = run_judge(bundle, caller=_mock_caller_polymorphism)
    verdict = result.verdict

    # Worker exits with code 0 — pytest framework's own exit code reflects
    # this assertion passing. (exit-code observable)
    assert verdict.verdict == "approved"

    # The verdict's `judges` field is exactly one IRI; that IRI resolves in
    # the bundle to a node typed dec:SpecProposal. (graph observable)
    assert isinstance(verdict.judges, str)
    assert verdict.judges == "urn:spec-proposal:REQ-007"
    assert _resolve_in_bundle(verdict.judges, bundle) == "dec:SpecProposal"

    # The verdict's `against` field is a list of exactly one IRI; that IRI
    # resolves to a node typed dec:Request. (graph observable)
    assert isinstance(verdict.against, list)
    assert len(verdict.against) == 1
    assert _resolve_in_bundle(verdict.against[0], bundle) == "dec:Request"
    assert verdict.against[0] == "urn:request:REQ-007"

    # Neither judges nor against is empty — satisfies the minCount=1
    # constraints of the ADR-074 SHACL shape on QualityVerdict.
    assert verdict.judges
    assert verdict.against

    # The verdict serialises cleanly through the QualityVerdict pydantic
    # model without missing-field errors. (stdout observable: the JSON
    # the worker emits round-trips through the model.)
    payload = json.loads(verdict.model_dump_json(exclude_none=True))
    reparsed = QualityVerdict.model_validate(payload)
    assert reparsed.judges == verdict.judges
    assert reparsed.against == verdict.against
    assert reparsed.bundle_hash == "poly1234"
