//! ProvenanceViolation type + Feedback artifact serialisation (FT-073).
//!
//! When [`Validator::validate`](crate::core::graph::shacl::Validator::validate)
//! refuses a write, GraphWriter emits a [`ProvenanceViolation`] back to the
//! caller and routes a matching Feedback artifact (class
//! `provenance-violation`) to the producing session via the FT-029 routing
//! table. This module owns the Rust struct, the typed wire form, and the
//! quad-emission helper that builds the Feedback triples.
//!
//! The Feedback artifact is written via the *underlying* `GraphWriter`
//! chokepoint (oxi-events) rather than the application-layer
//! `StreamWriter`. The violation feedback is a system-internal emission
//! about a malformed write — running it back through the same SHACL
//! pipeline that rejected the original write would either deadlock or
//! require the feedback artifact itself to carry its own motivational
//! provenance. FT-073 §"Violation routing" calls this exception out
//! explicitly.

use oxigraph::model::{GraphName, Literal, NamedNode, Quad};
use serde::{Deserialize, Serialize};

use crate::core::vocab::{
    feedback_class as feedback_iri_node, feedback_class_pred, in_stream, lifecycle_state,
    orchestration_graph, severity, source_session, target_role, IRI_DEC_FEEDBACK,
    IRI_DEC_LIFECYCLE_STATE,
};

const RDF_TYPE: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#type";

/// Feedback class literal for SHACL/provenance rejections (FT-073).
///
/// Slice-1 emission only — not registered in the controlled
/// `dec:feedbackClass` vocabulary (which is enforced by SHACL `sh:in`
/// per FT-028). The feedback artifact is emitted via the underlying
/// `GraphWriter` chokepoint bypassing the application-layer feedback
/// validator; consumers querying for provenance-violation feedback look
/// for `dec:feedbackClass "provenance-violation"` directly.
pub const PROVENANCE_VIOLATION_CLASS: &str = "provenance-violation";

/// Default target role for routing provenance-violation feedback
/// (FT-073 §"Violation routing"; spec calls out operator-curator for
/// boundary-rejection cases, spec-author for in-session ones).
pub const DEFAULT_TARGET_ROLE: &str = "spec-author";

/// Severity literal stamped on every provenance-violation feedback.
pub const VIOLATION_SEVERITY: &str = "critical";

/// One structural reason a write was refused.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum ViolationKind {
    /// Mechanical-provenance triple was missing — almost always an
    /// internal bug (the harness's session-completion handler did not
    /// hand a session record to GraphWriter, or the materialisation pass
    /// missed this subject).
    MissingMechanical {
        /// The PROV predicate (`prov:wasGeneratedBy`, `prov:wasAttributedTo`,
        /// `prov:generatedAtTime`) that was absent.
        predicate: String,
    },
    /// Artifact declared neither a motivational predicate nor
    /// BoundaryArtifact class membership — the load-bearing
    /// dual-provenance failure mode FT-073 exists to prevent.
    MissingMotivational,
    /// Artifact declared as `dec:BoundaryArtifact` (or subclass) but
    /// missing the required `dec:external_origin` literal.
    MissingBoundaryExternalOrigin,
}

impl ViolationKind {
    /// Human-readable shorthand for diagnostics / Feedback bodies.
    #[must_use]
    pub fn summary(&self) -> String {
        match self {
            Self::MissingMechanical { predicate } => {
                format!("mechanical provenance missing: <{predicate}>")
            }
            Self::MissingMotivational => {
                "motivational provenance missing and artifact is not a dec:BoundaryArtifact"
                    .to_string()
            }
            Self::MissingBoundaryExternalOrigin => {
                "dec:BoundaryArtifact missing required dec:external_origin literal".to_string()
            }
        }
    }
}

/// One structured violation. Multiple may be emitted per refused commit;
/// each carries the artifact IRI it was attached to plus the failure
/// detail. Round-trips through serde for cross-process diagnostics
/// (Python defensive validator agreement check).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProvenanceViolation {
    /// Stable IRI of the artifact that failed validation.
    pub artifact: String,
    /// `rdf:type` of the artifact, as declared in the refused delta.
    pub declared_type: String,
    /// Structured reason.
    pub kind: ViolationKind,
    /// The motivational predicate set the type accepts (per FT-070's
    /// catalog). Populated even on mechanical violations so consumers
    /// have full per-type context in one report.
    pub accepted_motivational_predicates: Vec<String>,
}

impl ProvenanceViolation {
    /// Construct a violation record.
    #[must_use]
    pub fn new(
        artifact: &NamedNode,
        declared_type: &str,
        kind: ViolationKind,
        accepted_motivational_predicates: Vec<String>,
    ) -> Self {
        Self {
            artifact: artifact.as_str().to_string(),
            declared_type: declared_type.to_string(),
            kind,
            accepted_motivational_predicates,
        }
    }

    /// Stable shorthand for log / Feedback evidence bodies.
    #[must_use]
    pub fn render(&self) -> String {
        let mut s = format!(
            "artifact <{}> (rdf:type <{}>): {}",
            self.artifact,
            self.declared_type,
            self.kind.summary()
        );
        if !self.accepted_motivational_predicates.is_empty() {
            s.push_str("\n  accepted motivational predicates:");
            for p in &self.accepted_motivational_predicates {
                s.push_str(&format!(" <{p}>"));
            }
        }
        s
    }
}

/// Build the Feedback artifact quads for an aggregate violation. The
/// quads are written to the orchestration named graph through the
/// underlying GraphWriter; the caller is responsible for stamping
/// `dec:inStream` if the active stream is known.
///
/// Required fields per `dec:Feedback` shape (FT-026 / ADR-022):
///   - `rdf:type dec:Feedback`
///   - `dec:feedbackClass "provenance-violation"`
///   - `dec:lifecycleState "produced"`
///   - `dec:targetRole "spec-author"` (default; operator-curator for boundary cases)
///   - `dec:evidence "<rendered violations>"`
///   - `dec:sourceSession <producing session IRI>`
///   - `dec:severity "critical"`
///   - `dec:inStream <active value stream>` (added by caller when known)
pub fn violation_feedback_quads(
    feedback_iri: &NamedNode,
    producing_session: &NamedNode,
    target_role_value: &str,
    violations: &[ProvenanceViolation],
    active_stream: Option<&NamedNode>,
) -> Vec<Quad> {
    let g: GraphName = orchestration_graph().into_owned().into();
    let mut quads = required_feedback_quads(feedback_iri, &g);
    quads.extend(routing_quads(
        feedback_iri,
        producing_session,
        target_role_value,
        &g,
    ));
    quads.push(evidence_quad(feedback_iri, violations, &g));
    quads.push(severity_quad(feedback_iri, &g));
    if let Some(stream) = active_stream {
        quads.push(in_stream_quad_for(feedback_iri, stream, g));
    }
    quads
}

/// rdf:type / feedbackClass / lifecycleState triples — invariant prefix
/// of every emitted Feedback artifact.
fn required_feedback_quads(feedback_iri: &NamedNode, g: &GraphName) -> Vec<Quad> {
    vec![
        Quad::new(
            feedback_iri.clone(),
            NamedNode::new_unchecked(RDF_TYPE),
            feedback_iri_node().into_owned(),
            g.clone(),
        ),
        Quad::new(
            feedback_iri.clone(),
            feedback_class_pred().into_owned(),
            Literal::new_simple_literal(PROVENANCE_VIOLATION_CLASS),
            g.clone(),
        ),
        Quad::new(
            feedback_iri.clone(),
            lifecycle_state().into_owned(),
            Literal::new_simple_literal("produced"),
            g.clone(),
        ),
    ]
}

/// targetRole + sourceSession — route the violation back to the producer.
fn routing_quads(
    feedback_iri: &NamedNode,
    producing_session: &NamedNode,
    target_role_value: &str,
    g: &GraphName,
) -> Vec<Quad> {
    vec![
        Quad::new(
            feedback_iri.clone(),
            target_role().into_owned(),
            Literal::new_simple_literal(target_role_value),
            g.clone(),
        ),
        Quad::new(
            feedback_iri.clone(),
            source_session().into_owned(),
            producing_session.clone(),
            g.clone(),
        ),
    ]
}

fn evidence_quad(
    feedback_iri: &NamedNode,
    violations: &[ProvenanceViolation],
    g: &GraphName,
) -> Quad {
    Quad::new(
        feedback_iri.clone(),
        crate::core::vocab::evidence().into_owned(),
        Literal::new_simple_literal(render_violations(violations)),
        g.clone(),
    )
}

fn severity_quad(feedback_iri: &NamedNode, g: &GraphName) -> Quad {
    Quad::new(
        feedback_iri.clone(),
        severity().into_owned(),
        Literal::new_simple_literal(VIOLATION_SEVERITY),
        g.clone(),
    )
}

fn in_stream_quad_for(feedback_iri: &NamedNode, stream: &NamedNode, g: GraphName) -> Quad {
    Quad::new(
        feedback_iri.clone(),
        in_stream().into_owned(),
        stream.clone(),
        g,
    )
}

/// Aggregate multiple violations into one evidence body. Each violation's
/// `render()` output is bullet-prefixed and newline-joined.
fn render_violations(violations: &[ProvenanceViolation]) -> String {
    let mut out = String::from("provenance violation(s) detected by FT-073 chokepoint:\n");
    for v in violations {
        out.push_str("  • ");
        out.push_str(&v.render());
        out.push('\n');
    }
    out
}

/// Confirm that the well-known IRIs this module relies on are still
/// linked at the expected positions in `core::vocab`. Compile-time
/// invariants — if the constants below disappear we want a build break,
/// not a runtime surprise.
#[allow(dead_code)]
const _ASSERT_VOCAB_PRESENT: &[&str] = &[IRI_DEC_FEEDBACK, IRI_DEC_LIFECYCLE_STATE];

#[cfg(test)]
mod tests {
    use super::*;
    use oxigraph::model::NamedNode;

    fn nn(s: &str) -> NamedNode {
        NamedNode::new_unchecked(s)
    }

    #[test]
    fn violation_renders_with_motivational_predicates() {
        let v = ProvenanceViolation::new(
            &nn("https://decision-cli.dev/ns/feature/x"),
            "https://decision-cli.dev/ns#Feature",
            ViolationKind::MissingMotivational,
            vec![
                "https://decision-cli.dev/ns#addresses".into(),
                "https://decision-cli.dev/ns#decomposesFrom".into(),
            ],
        );
        let rendered = v.render();
        assert!(rendered.contains("Feature"));
        assert!(rendered.contains("dec:BoundaryArtifact"));
        assert!(rendered.contains("addresses"));
    }

    #[test]
    fn feedback_quads_cover_required_shape() {
        let feedback = nn("https://decision-cli.dev/ns/feedback/violation-1");
        let session = nn("https://decision-cli.dev/ns/session/s1");
        let stream = nn("https://decision-cli.dev/ns/stream/example");
        let v = ProvenanceViolation::new(
            &nn("https://decision-cli.dev/ns/feature/x"),
            "https://decision-cli.dev/ns#Feature",
            ViolationKind::MissingMotivational,
            vec!["https://decision-cli.dev/ns#addresses".into()],
        );
        let quads = violation_feedback_quads(
            &feedback,
            &session,
            DEFAULT_TARGET_ROLE,
            &[v],
            Some(&stream),
        );
        // At least: rdf:type, feedbackClass, lifecycleState, targetRole,
        // evidence, sourceSession, severity, inStream = 8 quads.
        assert!(quads.len() >= 8, "got {}", quads.len());
        let predicates: Vec<&str> = quads.iter().map(|q| q.predicate.as_str()).collect();
        assert!(predicates.contains(&RDF_TYPE));
        assert!(predicates.contains(&crate::core::vocab::IRI_DEC_FEEDBACK_CLASS));
        assert!(predicates.contains(&crate::core::vocab::IRI_DEC_LIFECYCLE_STATE));
        assert!(predicates.contains(&crate::core::vocab::IRI_DEC_TARGET_ROLE));
        assert!(predicates.contains(&crate::core::vocab::IRI_DEC_EVIDENCE));
        assert!(predicates.contains(&crate::core::vocab::IRI_DEC_SOURCE_SESSION));
        assert!(predicates.contains(&crate::core::vocab::IRI_DEC_SEVERITY));
        assert!(predicates.contains(&crate::core::vocab::IRI_DEC_IN_STREAM));
    }
}
