//! In-memory `dec:ConformanceAudit` shape + RDF serialisation (FT-092 / ADR-055 / ADR-060).

use oxigraph::model::{GraphName, Literal, NamedNode, NamedNodeRef, Quad};

use crate::core::vocab::{
    artifact_class, audit_class_pred, audit_notes_pred, audits_pred, conformance_audit_class,
    generated_at_time_pred, was_attributed_to_pred, was_generated_by_pred,
    CONFORMANCE_AUDIT_AUTOMATED_REPLAY, CONFORMANCE_AUDIT_MANUAL_REVIEW,
    IRI_DEC_CONFORMANCE_AUDIT_PREFIX, IRI_XSD_DATE_TIME,
};

const RDF_TYPE: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#type";

/// Audit-class discriminator (ADR-055 / ADR-060).
///
/// Slice 1 (FT-092) only emits `ManualReview`. The `AutomatedReplay`
/// variant exists in the vocabulary so slice 2's conformance-replay
/// runner can produce audits against the same schema without an
/// artifact-type migration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ConformanceAuditClass {
    /// Curator hand-review per ADR-060.
    ManualReview,
    /// Conformance corpus replay (slice 2+).
    AutomatedReplay,
}

impl ConformanceAuditClass {
    /// Stable wire string for the audit class.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::ManualReview => CONFORMANCE_AUDIT_MANUAL_REVIEW,
            Self::AutomatedReplay => CONFORMANCE_AUDIT_AUTOMATED_REPLAY,
        }
    }

    /// Parse a class from its wire string. Unknown values resolve to
    /// `None` — the slice-2+ SHACL `sh:in` shape will reject them at
    /// write time.
    #[must_use]
    pub fn try_from_str(s: &str) -> Option<Self> {
        match s {
            CONFORMANCE_AUDIT_MANUAL_REVIEW => Some(Self::ManualReview),
            CONFORMANCE_AUDIT_AUTOMATED_REPLAY => Some(Self::AutomatedReplay),
            _ => None,
        }
    }
}

/// In-memory `dec:ConformanceAudit` artifact (FT-092 / ADR-055).
///
/// Identity is `id`; the canonical IRI is
/// `https://decision-cli.dev/ns/conformance-audit/<id>`. Two provenance
/// edges are mandatory (ADR-038 / ADR-039):
///
/// - **mechanical**: `prov:wasGeneratedBy` → action session;
///   `prov:wasAttributedTo` → agent (the producing role);
///   `prov:generatedAtTime` → RFC3339 timestamp.
/// - **motivational**: `dec:audits` → audited `dec:WorkerImage`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConformanceAudit {
    /// Stable id used for IRI minting.
    pub id: String,
    /// One of {`manual-review`, `automated-replay`}.
    pub audit_class: ConformanceAuditClass,
    /// `dec:audits` — IRI of the audited `dec:WorkerImage`.
    pub audits_image: NamedNode,
    /// Operator-facing notes capturing the audit's substance.
    /// For `manual-review` this is the Curator's rationale; for
    /// `automated-replay` it is the runner's summary.
    pub notes: String,
    /// `prov:wasGeneratedBy` — the producing Session IRI.
    pub generated_by_session: NamedNode,
    /// `prov:wasAttributedTo` — the producing Agent IRI.
    pub attributed_to_agent: NamedNode,
    /// `prov:generatedAtTime` — RFC3339 emission timestamp.
    pub generated_at_time: String,
}

impl ConformanceAudit {
    /// Construct the canonical IRI for this audit.
    #[must_use]
    pub fn iri(&self) -> NamedNode {
        NamedNode::new_unchecked(format!(
            "{prefix}{id}",
            prefix = IRI_DEC_CONFORMANCE_AUDIT_PREFIX,
            id = self.id,
        ))
    }

    /// Serialise the audit to RDF quads in the supplied named graph.
    ///
    /// Emits two `rdf:type` triples — `dec:ConformanceAudit` AND
    /// `dec:Artifact` — so the universal mechanical-provenance shape
    /// (FT-069 / ADR-038) recognises the artifact-class membership it
    /// targets.
    #[must_use]
    pub fn to_quads(&self, graph: NamedNodeRef<'_>) -> Vec<Quad> {
        let g: GraphName = graph.into_owned().into();
        let subject = self.iri();
        let mut quads = self.header_quads(&subject, &g);
        quads.extend(self.field_quads(&subject, &g));
        quads.extend(self.provenance_quads(&subject, &g));
        quads
    }

    fn header_quads(&self, subject: &NamedNode, g: &GraphName) -> Vec<Quad> {
        let rdf_type = NamedNodeRef::new_unchecked(RDF_TYPE);
        vec![
            Quad::new(
                subject.clone(),
                rdf_type,
                conformance_audit_class(),
                g.clone(),
            ),
            Quad::new(subject.clone(), rdf_type, artifact_class(), g.clone()),
        ]
    }

    fn field_quads(&self, subject: &NamedNode, g: &GraphName) -> Vec<Quad> {
        vec![
            literal_quad(subject, audit_class_pred(), self.audit_class.as_str(), g),
            literal_quad(subject, audit_notes_pred(), &self.notes, g),
        ]
    }

    fn provenance_quads(&self, subject: &NamedNode, g: &GraphName) -> Vec<Quad> {
        vec![
            named_quad(subject, audits_pred(), &self.audits_image, g),
            named_quad(
                subject,
                was_generated_by_pred(),
                &self.generated_by_session,
                g,
            ),
            named_quad(
                subject,
                was_attributed_to_pred(),
                &self.attributed_to_agent,
                g,
            ),
            datetime_quad(
                subject,
                generated_at_time_pred(),
                &self.generated_at_time,
                g,
            ),
        ]
    }
}

fn literal_quad(s: &NamedNode, p: NamedNodeRef<'_>, value: &str, g: &GraphName) -> Quad {
    Quad::new(
        s.clone(),
        p.into_owned(),
        Literal::new_simple_literal(value),
        g.clone(),
    )
}

fn datetime_quad(s: &NamedNode, p: NamedNodeRef<'_>, value: &str, g: &GraphName) -> Quad {
    Quad::new(
        s.clone(),
        p.into_owned(),
        Literal::new_typed_literal(value, NamedNode::new_unchecked(IRI_XSD_DATE_TIME)),
        g.clone(),
    )
}

fn named_quad(s: &NamedNode, p: NamedNodeRef<'_>, o: &NamedNode, g: &GraphName) -> Quad {
    Quad::new(s.clone(), p.into_owned(), o.clone(), g.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::vocab::{
        bundle_graph, IRI_DEC_AUDITS, IRI_DEC_AUDIT_CLASS, IRI_DEC_AUDIT_NOTES,
        IRI_PROV_GENERATED_AT_TIME, IRI_PROV_WAS_ATTRIBUTED_TO_MECHANICAL,
        IRI_PROV_WAS_GENERATED_BY,
    };

    fn fixture() -> ConformanceAudit {
        ConformanceAudit {
            id: "audit-001".to_string(),
            audit_class: ConformanceAuditClass::ManualReview,
            audits_image: NamedNode::new_unchecked(
                "https://decision-cli.dev/ns/worker-image/example/v1.0.0",
            ),
            notes: "Curator reviewed and approved.".to_string(),
            generated_by_session: NamedNode::new_unchecked(
                "https://decision-cli.dev/ns/session/curator-001",
            ),
            attributed_to_agent: NamedNode::new_unchecked(
                "https://decision-cli.dev/ns/agent/worker-curator",
            ),
            generated_at_time: "2026-05-26T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn class_round_trips() {
        for c in [
            ConformanceAuditClass::ManualReview,
            ConformanceAuditClass::AutomatedReplay,
        ] {
            assert_eq!(ConformanceAuditClass::try_from_str(c.as_str()), Some(c));
        }
        assert!(ConformanceAuditClass::try_from_str("self-audit").is_none());
    }

    #[test]
    fn iri_is_canonical() {
        let a = fixture();
        assert_eq!(
            a.iri().as_str(),
            "https://decision-cli.dev/ns/conformance-audit/audit-001"
        );
    }

    #[test]
    fn to_quads_includes_all_required_predicates() {
        let a = fixture();
        let quads = a.to_quads(bundle_graph());
        let predicates: Vec<&str> = quads.iter().map(|q| q.predicate.as_str()).collect();
        assert!(predicates.contains(&IRI_DEC_AUDIT_CLASS));
        assert!(predicates.contains(&IRI_DEC_AUDIT_NOTES));
        assert!(predicates.contains(&IRI_DEC_AUDITS));
        assert!(predicates.contains(&IRI_PROV_WAS_GENERATED_BY));
        assert!(predicates.contains(&IRI_PROV_WAS_ATTRIBUTED_TO_MECHANICAL));
        assert!(predicates.contains(&IRI_PROV_GENERATED_AT_TIME));
    }
}
