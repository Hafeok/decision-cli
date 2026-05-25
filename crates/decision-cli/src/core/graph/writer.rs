//! validate_and_commit chokepoint — FT-073 / ADR-041.
//!
//! Wraps the application-layer `StreamWriter` with the FT-073 dual-
//! provenance pipeline:
//!
//! 1. Optionally materialise the universal mechanical block on every
//!    artifact subject in the delta from the supplied `SessionAttribution`.
//!    Workers / feature code that already authored the mechanical block
//!    skip this step.
//! 2. Run the FT-073 [`Validator`] over the materialised delta.
//! 3. On conformance, commit through `StreamWriter::commit` (which runs
//!    the existing per-type SHACL validators + safety checks).
//! 4. On non-conformance, emit a Feedback artifact of class
//!    `provenance-violation` via the *underlying* `GraphWriter`
//!    chokepoint (bypassing `StreamWriter` so the violation feedback
//!    itself does not loop through this validator), and return a
//!    structured `Err` whose message starts with `provenance violation`
//!    so callers can match the prefix.
//!
//! The slice-1 surface is opt-in: existing call sites continue to use
//! `StreamWriter::commit`, while FT-073-aware paths use
//! [`validate_and_commit`] directly. ADR-041's "every mutation passes
//! through validation" property becomes the slice-2 cutover scenario
//! tracked by FT-074's migration plan.

use anyhow::{anyhow, Result};
use oxi_events::{CommitResult, Mutation};
use oxigraph::model::{NamedNode, Quad, Subject, Term};

use crate::core::graph::shacl::{ValidationReport, Validator};
use crate::core::graph::violation::{
    violation_feedback_quads, ProvenanceViolation, DEFAULT_TARGET_ROLE,
};
use crate::core::ontology::mechanical_provenance::{materialise_quads, SessionAttribution};
use crate::core::ontology::BOUNDARY_ARTIFACT_CLASS;
use crate::core::stream_writer::StreamWriter;
use crate::core::vocab::{
    orchestration_graph, IRI_PROV_GENERATED_AT_TIME, IRI_PROV_WAS_ATTRIBUTED_TO_MECHANICAL,
    IRI_PROV_WAS_GENERATED_BY,
};

const RDF_TYPE: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#type";

/// Structured outcome of [`validate_and_commit`] when validation fails.
///
/// Equivalent to FT-073 spec's `WriteError::ProvenanceRejected(violation)`:
/// callers translate to whatever local error type they surface (anyhow
/// throughout `core::stream_writer`; `WriteError` in slice-2 once the
/// typed-error refactor lands per `decision-cli-slice-1-bounds.md`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProvenanceRejection {
    /// All violations observed in the refused delta, in input order.
    pub violations: Vec<ProvenanceViolation>,
    /// IRI of the Feedback artifact emitted to the orchestration graph.
    pub emitted_feedback: NamedNode,
}

/// Behavioural knobs for [`validate_and_commit`].
#[derive(Debug, Clone)]
pub struct ValidateAndCommitOptions {
    /// When `Some`, the universal mechanical block is materialised from
    /// the attribution onto every artifact subject in the delta that
    /// lacks it. Set `None` to skip materialisation and validate the
    /// caller-supplied triples as-is.
    pub attribution: Option<SessionAttribution>,
    /// Stable IRI used for the Feedback artifact emitted on rejection.
    /// Required so callers can pass a deterministic IRI in tests; in
    /// production this is typically a UUIDv7-derived URN.
    pub feedback_iri: NamedNode,
    /// Producing session IRI — recorded on the Feedback artifact's
    /// `dec:sourceSession`. Usually the same IRI as `attribution.session`,
    /// kept separate so callers without an attribution can still route.
    pub producing_session: NamedNode,
    /// Target role literal. Defaults to `spec-author`; boundary-rejection
    /// flows override to `operator-curator` per FT-073 §"Violation routing".
    pub target_role: String,
}

impl ValidateAndCommitOptions {
    /// Slice-1 default constructor: materialise from `attribution`, route
    /// to `spec-author`.
    #[must_use]
    pub fn new(
        attribution: SessionAttribution,
        feedback_iri: NamedNode,
        producing_session: NamedNode,
    ) -> Self {
        Self {
            attribution: Some(attribution),
            feedback_iri,
            producing_session,
            target_role: DEFAULT_TARGET_ROLE.to_string(),
        }
    }

    /// Variant that skips materialisation — used when the caller (e.g. a
    /// worker SDK that already authored the mechanical block) wants
    /// pass-through validation only.
    #[must_use]
    pub fn pass_through(feedback_iri: NamedNode, producing_session: NamedNode) -> Self {
        Self {
            attribution: None,
            feedback_iri,
            producing_session,
            target_role: DEFAULT_TARGET_ROLE.to_string(),
        }
    }

    /// Override target role (e.g. `operator-curator` for boundary cases).
    #[must_use]
    pub fn with_target_role(mut self, role: impl Into<String>) -> Self {
        self.target_role = role.into();
        self
    }
}

/// FT-073 entry point. See module docs.
pub fn validate_and_commit(
    writer: &StreamWriter,
    validator: &Validator,
    mutation: Mutation,
    options: &ValidateAndCommitOptions,
) -> Result<CommitResult> {
    let materialised = materialise_if_requested(mutation, &options.attribution);
    let snapshot = writer.inner().store();
    let report = validator.validate(&materialised.inserts, Some(snapshot));
    if !report.conforms {
        let emitted = emit_violation_feedback(writer, &report, options)?;
        return Err(rejection_error(report, emitted));
    }
    writer.commit(materialised)
}

/// Public helper for callers that want the report without committing
/// (used by the test harness's dual-validator agreement check).
#[must_use]
pub fn validate_only(
    validator: &Validator,
    mutation: &Mutation,
    attribution: Option<&SessionAttribution>,
) -> ValidationReport {
    let cloned = Mutation::insert(mutation.inserts.iter().cloned());
    let materialised = materialise_if_requested(cloned, &attribution.cloned());
    validator.validate(&materialised.inserts, None)
}

fn materialise_if_requested(
    mut mutation: Mutation,
    attribution: &Option<SessionAttribution>,
) -> Mutation {
    let Some(attribution) = attribution else {
        return mutation;
    };
    let subjects = artifact_subjects_needing_mechanical(&mutation.inserts);
    for subject in subjects {
        let g = oxigraph::model::NamedNodeRef::new_unchecked(crate::core::vocab::IRI_DEC_GRAPH_ORCHESTRATION);
        for q in materialise_quads(&subject, attribution, g) {
            mutation.inserts.push(q);
        }
    }
    mutation
}

/// Collect every artifact subject in `inserts` that has a non-empty
/// `rdf:type` triple but lacks at least one mechanical-block predicate.
fn artifact_subjects_needing_mechanical(inserts: &[Quad]) -> Vec<NamedNode> {
    let mut all: Vec<NamedNode> = Vec::new();
    for q in inserts {
        if q.predicate.as_str() != RDF_TYPE {
            continue;
        }
        let Subject::NamedNode(s) = &q.subject else {
            continue;
        };
        let Term::NamedNode(cls) = &q.object else {
            continue;
        };
        // Skip BoundaryArtifact subclasses — they still need mechanical,
        // but they may also need an explicit external_origin literal that
        // the caller knows about. Materialisation only adds mechanical;
        // the existing external_origin literal is the caller's job.
        // (Materialisation is idempotent so leaving boundary subjects in
        // the set is harmless — but explicit early-out keeps the helper
        // predictable.)
        let _ = cls;
        if !all.iter().any(|x| x == s) {
            all.push(s.clone());
        }
    }
    all.into_iter()
        .filter(|s| needs_mechanical(inserts, s))
        .collect()
}

fn needs_mechanical(inserts: &[Quad], subject: &NamedNode) -> bool {
    let triples = [
        IRI_PROV_WAS_GENERATED_BY,
        IRI_PROV_WAS_ATTRIBUTED_TO_MECHANICAL,
        IRI_PROV_GENERATED_AT_TIME,
    ];
    triples.iter().any(|p| {
        !inserts
            .iter()
            .any(|q| q.predicate.as_str() == *p && matches_subject(&q.subject, subject))
    })
}

fn matches_subject(s: &Subject, t: &NamedNode) -> bool {
    matches!(s, Subject::NamedNode(n) if n == t)
}

/// Emit the Feedback artifact for the rejection. Writes through the
/// *underlying* `GraphWriter` so the violation feedback itself does not
/// loop back through `StreamWriter::commit`'s validators.
fn emit_violation_feedback(
    writer: &StreamWriter,
    report: &ValidationReport,
    options: &ValidateAndCommitOptions,
) -> Result<NamedNode> {
    let _ = orchestration_graph();
    let _ = BOUNDARY_ARTIFACT_CLASS; // keep import live; reserved for boundary-rejection routing
    let quads = violation_feedback_quads(
        &options.feedback_iri,
        &options.producing_session,
        &options.target_role,
        &report.violations,
        Some(writer.active_stream()),
    );
    let mutation = Mutation::insert(quads.iter().cloned())
        .with_cause(format!(
            "FT-073: emitting provenance-violation feedback for {} violation(s)",
            report.violations.len()
        ));
    writer
        .inner()
        .commit(mutation)
        .map_err(|err| anyhow!("failed to emit provenance-violation feedback: {err}"))?;
    Ok(options.feedback_iri.clone())
}

fn rejection_error(report: ValidationReport, feedback: NamedNode) -> anyhow::Error {
    let mut body = String::from("provenance violation: write refused by FT-073 chokepoint\n");
    for v in &report.violations {
        body.push_str("  • ");
        body.push_str(&v.render());
        body.push('\n');
    }
    body.push_str(&format!(
        "  ↳ emitted Feedback <{}> (class: provenance-violation)\n",
        feedback.as_str()
    ));
    anyhow!(body)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn options_default_target_role_is_spec_author() {
        let attribution = SessionAttribution::new(
            NamedNode::new_unchecked("https://decision-cli.dev/ns/session/s1"),
            vec![NamedNode::new_unchecked(
                "https://decision-cli.dev/ns/agent/a1",
            )],
            "2026-05-25T20:00:00Z",
        );
        let opts = ValidateAndCommitOptions::new(
            attribution,
            NamedNode::new_unchecked("https://decision-cli.dev/ns/feedback/v1"),
            NamedNode::new_unchecked("https://decision-cli.dev/ns/session/s1"),
        );
        assert_eq!(opts.target_role, DEFAULT_TARGET_ROLE);
    }
}
