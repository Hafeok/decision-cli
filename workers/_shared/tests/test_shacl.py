"""Defensive provenance validator tests (FT-073 §6 dual-validator agreement)."""

from __future__ import annotations

from _shared.shacl import (
    BOUNDARY_ARTIFACT_CLASS,
    EXTERNAL_ORIGIN_PROP,
    PROV_GENERATED_AT_TIME,
    PROV_WAS_ATTRIBUTED_TO,
    PROV_WAS_GENERATED_BY,
    RDF_TYPE,
    Triple,
    parse_nquads,
    validate,
)

NS_DEC = "https://decision-cli.dev/ns#"
FEATURE_IRI = f"{NS_DEC}Feature"
FEATURE_INSTANCE = f"{NS_DEC.replace('#', '/')}feature/sample"
SESSION_IRI = f"{NS_DEC.replace('#', '/')}session/s1"
AGENT_IRI = f"{NS_DEC.replace('#', '/')}agent/a1"
TS = "2026-05-25T20:00:00Z"


def _mechanical(subject: str) -> list[Triple]:
    return [
        Triple(subject, PROV_WAS_GENERATED_BY, SESSION_IRI, "iri"),
        Triple(subject, PROV_WAS_ATTRIBUTED_TO, AGENT_IRI, "iri"),
        Triple(subject, PROV_GENERATED_AT_TIME, TS, "literal"),
    ]


def test_rejects_feature_missing_motivational():
    triples = [Triple(FEATURE_INSTANCE, RDF_TYPE, FEATURE_IRI, "iri")]
    triples.extend(_mechanical(FEATURE_INSTANCE))
    report = validate(triples)
    assert not report.conforms
    assert any(
        v.kind == "MissingMotivational" and v.artifact == FEATURE_INSTANCE
        for v in report.violations
    )


def test_accepts_feature_with_motivational():
    triples = [Triple(FEATURE_INSTANCE, RDF_TYPE, FEATURE_IRI, "iri")]
    triples.extend(_mechanical(FEATURE_INSTANCE))
    triples.append(
        Triple(
            FEATURE_INSTANCE,
            f"{NS_DEC}addresses",
            f"{NS_DEC.replace('#', '/')}feedback/fb1",
            "iri",
        )
    )
    report = validate(triples)
    assert report.conforms, report.violations


def test_accepts_boundary_feature_with_external_origin():
    iri = f"{NS_DEC.replace('#', '/')}feature/boundary"
    triples = [
        Triple(iri, RDF_TYPE, FEATURE_IRI, "iri"),
        Triple(iri, RDF_TYPE, BOUNDARY_ARTIFACT_CLASS, "iri"),
        Triple(iri, EXTERNAL_ORIGIN_PROP, "chat://t-2026-05-25", "literal"),
    ]
    triples.extend(_mechanical(iri))
    report = validate(triples)
    assert report.conforms, report.violations


def test_rejects_missing_mechanical_block():
    triples = [
        Triple(FEATURE_INSTANCE, RDF_TYPE, FEATURE_IRI, "iri"),
        Triple(
            FEATURE_INSTANCE,
            f"{NS_DEC}addresses",
            f"{NS_DEC.replace('#', '/')}feedback/fb1",
            "iri",
        ),
    ]
    report = validate(triples)
    assert not report.conforms
    mechanical = [v for v in report.violations if v.kind == "MissingMechanical"]
    assert len(mechanical) == 3


def test_parse_nquads_round_trips_the_failing_delta():
    # Mirrors `sample_negative_nquads()` in the Rust integration test —
    # FT-073 §6 dual-validator agreement: both sides see the same input.
    g = "https://decision-cli.dev/ns/orchestration"
    blob = "\n".join(
        [
            f"<{FEATURE_INSTANCE}> <{RDF_TYPE}> <{FEATURE_IRI}> <{g}> .",
            f"<{FEATURE_INSTANCE}> <{PROV_WAS_GENERATED_BY}> <{SESSION_IRI}> <{g}> .",
            f"<{FEATURE_INSTANCE}> <{PROV_WAS_ATTRIBUTED_TO}> <{AGENT_IRI}> <{g}> .",
            (
                f"<{FEATURE_INSTANCE}> <{PROV_GENERATED_AT_TIME}> "
                f'"{TS}"^^<http://www.w3.org/2001/XMLSchema#dateTime> <{g}> .'
            ),
        ]
    )
    triples = parse_nquads(blob)
    report = validate(triples)
    assert not report.conforms
    assert any(v.kind == "MissingMotivational" for v in report.violations)
