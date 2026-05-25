"""Defensive provenance validator for the worker SDK (FT-073)."""

from __future__ import annotations

import json
from dataclasses import dataclass, field
from typing import Dict, Iterable, List, Optional, Set, Tuple

# --- Slice-1 type-shape table (mirrors `core::graph::shacl::table`) ---------
#
# The Rust validator is authoritative (FT-073 §6 — harness re-validates and
# wins). This Python copy lets workers reject obviously-wrong artifacts
# before handing them across the ADR-008 worker contract, keeping the
# violation feedback loop short.

NS_DEC = "https://decision-cli.dev/ns#"
NS_PROV = "http://www.w3.org/ns/prov#"
NS_RDF = "http://www.w3.org/1999/02/22-rdf-syntax-ns#"
NS_XSD = "http://www.w3.org/2001/XMLSchema#"

RDF_TYPE = f"{NS_RDF}type"
PROV_WAS_GENERATED_BY = f"{NS_PROV}wasGeneratedBy"
PROV_WAS_ATTRIBUTED_TO = f"{NS_PROV}wasAttributedTo"
PROV_GENERATED_AT_TIME = f"{NS_PROV}generatedAtTime"

BOUNDARY_ARTIFACT_CLASS = f"{NS_DEC}BoundaryArtifact"
BOUNDARY_ARTIFACT_SUBCLASSES = (
    f"{NS_DEC}SensingActionOutput",
    f"{NS_DEC}InitialRequest",
    f"{NS_DEC}BootstrapArtifact",
    f"{NS_DEC}MigrationBackfill",
)
EXTERNAL_ORIGIN_PROP = f"{NS_DEC}external_origin"

# (class shortname, motivational predicate shortnames, accepts_boundary, motivational_exempt)
_PER_TYPE_TABLE: Tuple[Tuple[str, Tuple[str, ...], bool, bool], ...] = (
    ("Acknowledgement", ("motivatedBy",), True, False),
    ("ADR", ("addresses", "decidesFor", "supersedes"), True, False),
    ("Brief", ("respondsTo",), True, False),
    ("ConformanceAudit", ("audits",), True, False),
    ("Dependency", ("requiredBy",), True, False),
    ("DiscoveryFinding", ("derivedFrom",), True, False),
    ("Dispatch", (), False, True),
    (
        "Feature",
        ("addresses", "decomposesFrom", "originatedFrom", "respondsTo"),
        True,
        False,
    ),
    ("Feedback", ("observedIn", "observedVia", "producedBy"), True, False),
    ("Model", ("addresses", "decomposesFrom"), True, False),
    ("Policy", ("addresses", "decomposesFrom"), True, False),
    ("QueryTemplate", ("decomposesFrom", "addresses"), True, False),
    ("Question", ("raisedIn", "raisedBy"), True, False),
    ("Session", (), False, True),
    ("Subscription", ("motivatedBy",), True, False),
    ("TC", ("validates",), True, False),
    ("WorkerImage", ("addresses", "decomposesFrom"), True, False),
    ("WorkerImageSubmission", (), True, False),
)


@dataclass(frozen=True)
class TypeShape:
    """One per-type provenance shape — slice-1 mirror of FT-072's TTL."""

    class_iri: str
    motivational: frozenset[str]
    accepts_boundary: bool
    motivational_exempt: bool


def _build_table() -> Dict[str, TypeShape]:
    out: Dict[str, TypeShape] = {}
    for short, preds, accepts_boundary, motivational_exempt in _PER_TYPE_TABLE:
        iri = f"{NS_DEC}{short}"
        out[iri] = TypeShape(
            class_iri=iri,
            motivational=frozenset(f"{NS_DEC}{p}" for p in preds),
            accepts_boundary=accepts_boundary,
            motivational_exempt=motivational_exempt,
        )
    return out


_TYPE_SHAPES = _build_table()
_BOUNDARY_CLASS_SET: Set[str] = {BOUNDARY_ARTIFACT_CLASS, *BOUNDARY_ARTIFACT_SUBCLASSES}


@dataclass
class ProvenanceViolation:
    """One structural reason a write would be refused."""

    artifact: str
    declared_type: str
    kind: str  # "missing-mechanical" | "missing-motivational" | "missing-boundary-external-origin"
    detail: str
    accepted_motivational_predicates: List[str] = field(default_factory=list)

    def as_dict(self) -> Dict[str, object]:
        return {
            "artifact": self.artifact,
            "declared_type": self.declared_type,
            "kind": self.kind,
            "detail": self.detail,
            "accepted_motivational_predicates": list(self.accepted_motivational_predicates),
        }


@dataclass
class ValidationReport:
    conforms: bool
    violations: List[ProvenanceViolation]

    def as_dict(self) -> Dict[str, object]:
        return {
            "conforms": self.conforms,
            "violations": [v.as_dict() for v in self.violations],
        }


# --- Triple shape: minimal in-memory representation -------------------------


@dataclass
class Triple:
    """N-Quads-derived triple. `obj_kind` is "iri" or "literal"."""

    subject: str
    predicate: str
    obj: str
    obj_kind: str  # "iri" | "literal"


def parse_nquads(blob: str) -> List[Triple]:
    """Tiny N-Quads parser scoped to the FT-073 delta payloads.

    Workers feed in their about-to-be-emitted triple set as N-Quads. The
    parser is intentionally narrow — it accepts a subset sufficient for
    the validator's needs (IRI subjects, IRI predicates, IRI or literal
    objects). Production callers should prefer pyoxigraph if available;
    this parser exists so the validator runs in a worker venv that
    cannot install native deps.
    """
    triples: List[Triple] = []
    for raw_line in blob.splitlines():
        line = raw_line.strip()
        if not line or line.startswith("#"):
            continue
        if line.endswith("."):
            line = line[:-1].rstrip()
        triple = _parse_triple_line(line)
        if triple is not None:
            triples.append(triple)
    return triples


def _parse_triple_line(line: str) -> Optional[Triple]:
    """Best-effort parse of one N-Quads line (subject, predicate, object).

    Returns None for lines we cannot tokenise — the caller treats unparsed
    lines as silently dropped because the validator's job is not to be a
    general parser. Production paths must use pyoxigraph.
    """
    s, rest = _take_iri(line)
    if s is None:
        return None
    p, rest = _take_iri(rest)
    if p is None:
        return None
    rest = rest.lstrip()
    if rest.startswith("<"):
        o, _ = _take_iri(rest)
        if o is None:
            return None
        return Triple(s, p, o, "iri")
    if rest.startswith('"'):
        value, _ = _take_literal(rest)
        if value is None:
            return None
        return Triple(s, p, value, "literal")
    return None


def _take_iri(s: str) -> Tuple[Optional[str], str]:
    s = s.lstrip()
    if not s.startswith("<"):
        return None, s
    end = s.find(">")
    if end < 0:
        return None, s
    return s[1:end], s[end + 1 :]


def _take_literal(s: str) -> Tuple[Optional[str], str]:
    if not s.startswith('"'):
        return None, s
    # Find the closing quote, naively (no escape handling — slice-1).
    end = s.find('"', 1)
    if end < 0:
        return None, s
    return s[1:end], s[end + 1 :]


# --- Validator --------------------------------------------------------------


def validate(triples: Iterable[Triple]) -> ValidationReport:
    """Run the FT-073 validator over an N-Quads-derived triple set."""
    triples = list(triples)
    subjects = _artifact_subjects(triples)
    violations: List[ProvenanceViolation] = []
    for subject, types in subjects.items():
        for declared_type in types:
            _validate_one(triples, subject, declared_type, violations)
    return ValidationReport(conforms=not violations, violations=violations)


def _artifact_subjects(triples: List[Triple]) -> Dict[str, Set[str]]:
    out: Dict[str, Set[str]] = {}
    for t in triples:
        if t.predicate != RDF_TYPE or t.obj_kind != "iri":
            continue
        out.setdefault(t.subject, set()).add(t.obj)
    return out


def _validate_one(
    triples: List[Triple],
    subject: str,
    declared_type: str,
    out: List[ProvenanceViolation],
) -> None:
    shape = _TYPE_SHAPES.get(declared_type)
    _check_mechanical(triples, subject, declared_type, shape, out)
    if shape is None:
        return
    if shape.motivational_exempt:
        return
    _check_motivational(triples, subject, declared_type, shape, out)


def _check_mechanical(
    triples: List[Triple],
    subject: str,
    declared_type: str,
    shape: Optional[TypeShape],
    out: List[ProvenanceViolation],
) -> None:
    required = (
        PROV_WAS_GENERATED_BY,
        PROV_WAS_ATTRIBUTED_TO,
        PROV_GENERATED_AT_TIME,
    )
    motivational = sorted(shape.motivational) if shape else []
    for predicate in required:
        if _has_any_value(triples, subject, predicate):
            continue
        out.append(
            ProvenanceViolation(
                artifact=subject,
                declared_type=declared_type,
                kind="MissingMechanical",
                detail=f"missing required <{predicate}>",
                accepted_motivational_predicates=motivational,
            )
        )


def _check_motivational(
    triples: List[Triple],
    subject: str,
    declared_type: str,
    shape: TypeShape,
    out: List[ProvenanceViolation],
) -> None:
    if shape.accepts_boundary and _is_boundary(triples, subject):
        if not _has_any_value(triples, subject, EXTERNAL_ORIGIN_PROP):
            out.append(
                ProvenanceViolation(
                    artifact=subject,
                    declared_type=declared_type,
                    kind="MissingBoundaryExternalOrigin",
                    detail="dec:BoundaryArtifact missing required dec:external_origin literal",
                    accepted_motivational_predicates=sorted(shape.motivational),
                )
            )
        return
    if any(_has_any_value(triples, subject, p) for p in shape.motivational):
        return
    out.append(
        ProvenanceViolation(
            artifact=subject,
            declared_type=declared_type,
            kind="MissingMotivational",
            detail=(
                "motivational provenance missing and artifact is not a "
                "dec:BoundaryArtifact"
            ),
            accepted_motivational_predicates=sorted(shape.motivational),
        )
    )


def _has_any_value(triples: List[Triple], subject: str, predicate: str) -> bool:
    return any(t.subject == subject and t.predicate == predicate for t in triples)


def _is_boundary(triples: List[Triple], subject: str) -> bool:
    for t in triples:
        if t.subject != subject or t.predicate != RDF_TYPE or t.obj_kind != "iri":
            continue
        if t.obj in _BOUNDARY_CLASS_SET:
            return True
    return False


def validate_to_json(triples_blob: str) -> str:
    """CLI helper — read N-Quads from `triples_blob`, return JSON report."""
    triples = parse_nquads(triples_blob)
    report = validate(triples)
    return json.dumps(report.as_dict(), indent=2, sort_keys=True)
