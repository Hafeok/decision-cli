//! Provenance SHACL validator (FT-073 / ADR-041).
//!
//! Slice-1 chokepoint enforcement that turns the dual-provenance discipline
//! (ADR-038) from "shape files exist" into "the graph maintains the
//! invariant continuously". The [`Validator`] is constructed once at
//! `dec init` from the loaded shape catalog and reused for the lifetime of
//! the process — shape files are immutable per ADR-041's load model.
//!
//! Public surface:
//!
//! - [`Validator::load`] returns a process-shared instance with the per-
//!   type motivational predicate catalog and the BoundaryArtifact class
//!   set pre-computed.
//! - [`Validator::validate`] runs against the incoming triple delta
//!   composed with an optional read-snapshot. Returns a structured
//!   [`ValidationReport`] enumerating any violations (mechanical OR
//!   motivational) per artifact subject.
//!
//! Implementation detail: the slice-1 validator is hand-rolled rather
//! than wrapping an oxigraph-shacl crate. The enforcement surface is
//! narrow (mechanical block + motivational presence + boundary exemption
//! with subclass-reasoning) and a Rust-native validator matches the
//! existing per-shape validators elsewhere in `core::`. The Python
//! defensive validator (`workers/_shared/shacl.py`) mirrors the same
//! decision tree so the dual-validator agreement check (FT-073 §7) can
//! diff their verdicts deterministically.

mod table;

#[cfg(test)]
mod tests;

use std::collections::{BTreeMap, BTreeSet};

use oxigraph::model::{NamedNode, Quad, Subject, Term};
use oxigraph::store::Store;

use crate::core::graph::violation::{ProvenanceViolation, ViolationKind};
use crate::core::ontology::{
    OntologyHandle, BOUNDARY_ARTIFACT_CLASS, BOUNDARY_ARTIFACT_SUBCLASSES, EXTERNAL_ORIGIN_PROP,
};
use crate::core::vocab::{
    IRI_PROV_GENERATED_AT_TIME, IRI_PROV_WAS_ATTRIBUTED_TO_MECHANICAL, IRI_PROV_WAS_GENERATED_BY,
};

pub use table::{slice1_boundary_class_set, slice1_type_shapes, TypeShape};

/// `rdf:type` IRI literal (re-declared so this module stays import-clean
/// against the existing `core::vocab` surface).
pub const RDF_TYPE: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#type";

/// Structured result of [`Validator::validate`]. `conforms == true` iff
/// `violations` is empty.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ValidationReport {
    /// True iff every artifact in the delta passes mechanical + motivational checks.
    pub conforms: bool,
    /// Per-artifact violations in input order.
    pub violations: Vec<ProvenanceViolation>,
}

/// FT-073 dual-provenance validator over the shape catalog.
///
/// Cheap to clone (internal state is a small set of strings); call
/// [`Validator::load`] once at bootstrap and share via `Arc`.
#[derive(Debug, Clone)]
pub struct Validator {
    /// Per-type provenance shape table, keyed by `sh:targetClass` IRI.
    type_shapes: BTreeMap<String, TypeShape>,
    /// IRIs admitted as BoundaryArtifact under subclass reasoning
    /// (`dec:BoundaryArtifact` + the four slice-1 subclasses).
    boundary_class_set: BTreeSet<String>,
}

impl Validator {
    /// Load the slice-1 validator from the embedded shape catalog. Errors
    /// surface as [`ValidatorError::ShapeLoadFailure`] so `dec init` can
    /// fail fast per ADR-041's load-time fatal mode.
    pub fn load() -> Result<Self, ValidatorError> {
        let _ontology = OntologyHandle::load()
            .map_err(|err| ValidatorError::ShapeLoadFailure(err.to_string()))?;
        let type_shapes = slice1_type_shapes();
        let boundary_class_set =
            slice1_boundary_class_set(BOUNDARY_ARTIFACT_CLASS, BOUNDARY_ARTIFACT_SUBCLASSES);
        Ok(Self {
            type_shapes,
            boundary_class_set,
        })
    }

    /// Validate `delta` against the loaded shape catalog. `snapshot`, when
    /// present, is consulted for cross-artifact constraints — slice-1
    /// reserves that for the future fitness check; FT-073's slice-1
    /// surface only inspects the delta itself.
    #[must_use]
    pub fn validate(&self, delta: &[Quad], _snapshot: Option<&Store>) -> ValidationReport {
        let mut violations: Vec<ProvenanceViolation> = Vec::new();
        let subjects = artifact_subjects(delta);
        for (subject, types) in &subjects {
            for ty in types {
                self.validate_one(delta, subject, ty, &mut violations);
            }
        }
        ValidationReport {
            conforms: violations.is_empty(),
            violations,
        }
    }

    /// Per-type / per-subject validation step. Mechanical is always
    /// checked; motivational is type-driven (and skipped for the two
    /// FT-072 exempt shapes — Session, Dispatch).
    fn validate_one(
        &self,
        delta: &[Quad],
        subject: &NamedNode,
        declared_type: &str,
        out: &mut Vec<ProvenanceViolation>,
    ) {
        let shape = self.type_shapes.get(declared_type);
        self.check_mechanical(delta, subject, declared_type, shape, out);
        let Some(shape) = shape else { return };
        if shape.motivational_exempt {
            return;
        }
        self.check_motivational(delta, subject, declared_type, shape, out);
    }

    /// Mechanical-block check for one artifact subject. Surfaces violations
    /// as ProvenanceViolation entries with kind = MissingMechanical and
    /// the failing PROV predicate path.
    fn check_mechanical(
        &self,
        delta: &[Quad],
        subject: &NamedNode,
        declared_type: &str,
        shape: Option<&TypeShape>,
        out: &mut Vec<ProvenanceViolation>,
    ) {
        let triples = [
            IRI_PROV_WAS_GENERATED_BY,
            IRI_PROV_WAS_ATTRIBUTED_TO_MECHANICAL,
            IRI_PROV_GENERATED_AT_TIME,
        ];
        for predicate in triples {
            if has_any_value(delta, subject, predicate) {
                continue;
            }
            out.push(ProvenanceViolation::new(
                subject,
                declared_type,
                ViolationKind::MissingMechanical {
                    predicate: predicate.to_string(),
                },
                motivational_predicates_for(shape),
            ));
        }
    }

    /// Motivational-block check. Boundary-class membership short-circuits;
    /// otherwise one of the per-type predicates must be present.
    fn check_motivational(
        &self,
        delta: &[Quad],
        subject: &NamedNode,
        declared_type: &str,
        shape: &TypeShape,
        out: &mut Vec<ProvenanceViolation>,
    ) {
        if shape.accepts_boundary && self.is_boundary(delta, subject) {
            self.check_boundary_external_origin(delta, subject, declared_type, shape, out);
            return;
        }
        let has_motivational = shape
            .motivational
            .iter()
            .any(|pred| has_any_value(delta, subject, pred));
        if has_motivational {
            return;
        }
        out.push(ProvenanceViolation::new(
            subject,
            declared_type,
            ViolationKind::MissingMotivational,
            shape.motivational.iter().cloned().collect(),
        ));
    }

    /// Boundary artifacts must additionally carry `dec:external_origin`.
    fn check_boundary_external_origin(
        &self,
        delta: &[Quad],
        subject: &NamedNode,
        declared_type: &str,
        shape: &TypeShape,
        out: &mut Vec<ProvenanceViolation>,
    ) {
        if has_any_value(delta, subject, EXTERNAL_ORIGIN_PROP) {
            return;
        }
        out.push(ProvenanceViolation::new(
            subject,
            declared_type,
            ViolationKind::MissingBoundaryExternalOrigin,
            shape.motivational.iter().cloned().collect(),
        ));
    }

    /// True iff `subject` declares any class in [`Self::boundary_class_set`]
    /// in the delta. Subclass reasoning is handled by enumerating the
    /// known subclass set rather than running a SPARQL transitive walk
    /// (slice-1 keeps the validator hot-path free of store access).
    fn is_boundary(&self, delta: &[Quad], subject: &NamedNode) -> bool {
        for q in delta {
            if q.predicate.as_str() != RDF_TYPE {
                continue;
            }
            if !matches_subject(&q.subject, subject) {
                continue;
            }
            let Term::NamedNode(cls) = &q.object else {
                continue;
            };
            if self.boundary_class_set.contains(cls.as_str()) {
                return true;
            }
        }
        false
    }

    /// Expose the per-type motivational predicate set so the test harness
    /// can cross-check the table against the shape TTL files. Returns
    /// `None` when the type has no per-type shape registered.
    #[must_use]
    pub fn motivational_predicates(&self, type_iri: &str) -> Option<Vec<String>> {
        self.type_shapes
            .get(type_iri)
            .map(|shape| shape.motivational.iter().cloned().collect())
    }
}

/// Errors surfaced by [`Validator::load`] — fatal at `dec init`.
#[derive(Debug, thiserror::Error)]
pub enum ValidatorError {
    /// Embedded shape files failed to load or parse.
    #[error("shape load failure: {0}")]
    ShapeLoadFailure(String),
}

/// Collect every artifact subject in `delta` with a non-empty `rdf:type`
/// set. Returned via a BTreeMap so per-subject iteration is deterministic
/// for diagnostics.
fn artifact_subjects(delta: &[Quad]) -> BTreeMap<NamedNode, BTreeSet<String>> {
    let mut out: BTreeMap<NamedNode, BTreeSet<String>> = BTreeMap::new();
    for q in delta {
        if q.predicate.as_str() != RDF_TYPE {
            continue;
        }
        let Subject::NamedNode(s) = &q.subject else {
            continue;
        };
        let Term::NamedNode(cls) = &q.object else {
            continue;
        };
        out.entry(s.clone())
            .or_default()
            .insert(cls.as_str().to_string());
    }
    out
}

fn motivational_predicates_for(shape: Option<&TypeShape>) -> Vec<String> {
    shape
        .map(|s| s.motivational.iter().cloned().collect())
        .unwrap_or_default()
}

fn has_any_value(delta: &[Quad], subject: &NamedNode, predicate: &str) -> bool {
    delta
        .iter()
        .any(|q| q.predicate.as_str() == predicate && matches_subject(&q.subject, subject))
}

fn matches_subject(subject: &Subject, target: &NamedNode) -> bool {
    match subject {
        Subject::NamedNode(s) => s == target,
        _ => false,
    }
}
