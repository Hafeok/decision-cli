//! In-memory `dec:SignatureVerdict` artifact with RDF serialisation.
//!
//! FT-090 / ADR-017 / ADR-018: the SignatureVerdict is the interpretation
//! artifact paired with the identity-verifier action's pure-execution side.
//! Carries the [`SignatureVerdictClass`] discriminator, the operator-facing
//! rationale prose, the action session that generated it (mechanical
//! provenance per ADR-038), and the WorkerImageSubmission the verdict
//! responds to (motivational provenance per ADR-039: `dec:respondsTo` is a
//! `prov:wasDerivedFrom` sub-property).

use oxigraph::model::{GraphName, Literal, NamedNode, NamedNodeRef, Quad};

use crate::core::vocab::{
    artifact_class, generated_at_time_pred, responds_to_pred, signature_verdict_class,
    signature_verdict_class_pred, verdict_rationale_pred, was_attributed_to_pred,
    was_generated_by_pred, IRI_DEC_SIGNATURE_VERDICT_PREFIX, IRI_XSD_DATE_TIME,
    SIGNATURE_VERDICT_IMAGE_NOT_FOUND, SIGNATURE_VERDICT_INVALID_SIGNATURE,
    SIGNATURE_VERDICT_REKOR_ENTRY_MISSING, SIGNATURE_VERDICT_UNTRUSTED_IDENTITY,
    SIGNATURE_VERDICT_VALID,
};

const RDF_TYPE: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#type";

/// One of the five disjoint outcome classes FT-090 §Scope enumerates.
///
/// The variant order matches the order in which classes are documented in the
/// feature_spec, not classifier evaluation order (which is encoded in
/// [`super::classify`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SignatureVerdictClass {
    /// Signature checks, identity on trust list, Rekor inclusion confirmed.
    Valid,
    /// `cosign verify` failed cryptographically.
    InvalidSignature,
    /// Signature valid but signer not on trust list.
    UntrustedIdentity,
    /// Registry returned 404 for the candidate ref.
    ImageNotFound,
    /// Referenced Rekor entry doesn't exist or doesn't match.
    RekorEntryMissing,
}

impl SignatureVerdictClass {
    /// Stable wire string for the verdict class.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Valid => SIGNATURE_VERDICT_VALID,
            Self::InvalidSignature => SIGNATURE_VERDICT_INVALID_SIGNATURE,
            Self::UntrustedIdentity => SIGNATURE_VERDICT_UNTRUSTED_IDENTITY,
            Self::ImageNotFound => SIGNATURE_VERDICT_IMAGE_NOT_FOUND,
            Self::RekorEntryMissing => SIGNATURE_VERDICT_REKOR_ENTRY_MISSING,
        }
    }

    /// Parse a verdict class from its wire string. Returns `None` for unknown
    /// inputs — the SHACL shape (slice 2+) rejects them at commit time.
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            SIGNATURE_VERDICT_VALID => Some(Self::Valid),
            SIGNATURE_VERDICT_INVALID_SIGNATURE => Some(Self::InvalidSignature),
            SIGNATURE_VERDICT_UNTRUSTED_IDENTITY => Some(Self::UntrustedIdentity),
            SIGNATURE_VERDICT_IMAGE_NOT_FOUND => Some(Self::ImageNotFound),
            SIGNATURE_VERDICT_REKOR_ENTRY_MISSING => Some(Self::RekorEntryMissing),
            _ => None,
        }
    }
}

/// In-memory `dec:SignatureVerdict` artifact.
///
/// Identity is `id`; the canonical IRI is
/// `https://decision-cli.dev/ns/signature-verdict/<id>`. Two provenance
/// edges are mandatory (ADR-038 / ADR-039):
///
/// - **mechanical**: `prov:wasGeneratedBy` → action session;
///   `prov:wasAttributedTo` → agent (the identity-verifier role);
///   `prov:generatedAtTime` → RFC3339 timestamp.
/// - **motivational**: `dec:respondsTo` → originating
///   `dec:WorkerImageSubmission`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignatureVerdict {
    /// Stable id used for IRI minting.
    pub id: String,
    /// The five-value outcome class.
    pub verdict_class: SignatureVerdictClass,
    /// Operator-facing rationale prose; non-empty.
    pub rationale: String,
    /// `prov:wasGeneratedBy` — the action session IRI.
    pub generated_by_session: NamedNode,
    /// `prov:wasAttributedTo` — the agent IRI (the role / worker that produced the verdict).
    pub attributed_to_agent: NamedNode,
    /// `prov:generatedAtTime` — RFC3339 timestamp of verdict emission.
    pub generated_at_time: String,
    /// `dec:respondsTo` — the WorkerImageSubmission that motivated the verdict.
    pub responds_to_submission: NamedNode,
}

impl SignatureVerdict {
    /// Construct the canonical IRI for this verdict.
    #[must_use]
    pub fn iri(&self) -> NamedNode {
        NamedNode::new_unchecked(format!(
            "{prefix}{id}",
            prefix = IRI_DEC_SIGNATURE_VERDICT_PREFIX,
            id = self.id,
        ))
    }

    /// Serialise the verdict to RDF quads in the supplied named graph.
    /// Emits two `rdf:type` triples — `dec:SignatureVerdict` AND
    /// `dec:Artifact` — so the universal mechanical-provenance shape
    /// (FT-069 / ADR-038) sees the artifact-class membership it targets.
    #[must_use]
    pub fn to_quads(&self, graph: NamedNodeRef<'_>) -> Vec<Quad> {
        let g: GraphName = graph.into_owned().into();
        let subject = self.iri();
        let mut quads = self.header_quads(&subject, &g);
        quads.extend(self.mechanical_quads(&subject, &g));
        quads.push(self.motivational_quad(&subject, &g));
        quads
    }

    fn header_quads(&self, subject: &NamedNode, g: &GraphName) -> Vec<Quad> {
        let rdf_type = NamedNodeRef::new_unchecked(RDF_TYPE);
        vec![
            Quad::new(
                subject.clone(),
                rdf_type,
                signature_verdict_class(),
                g.clone(),
            ),
            Quad::new(subject.clone(), rdf_type, artifact_class(), g.clone()),
            literal_quad(
                subject,
                signature_verdict_class_pred(),
                self.verdict_class.as_str(),
                g,
            ),
            literal_quad(subject, verdict_rationale_pred(), &self.rationale, g),
        ]
    }

    fn mechanical_quads(&self, subject: &NamedNode, g: &GraphName) -> Vec<Quad> {
        vec![
            named_quad(subject, was_generated_by_pred(), &self.generated_by_session, g),
            named_quad(
                subject,
                was_attributed_to_pred(),
                &self.attributed_to_agent,
                g,
            ),
            datetime_quad(subject, generated_at_time_pred(), &self.generated_at_time, g),
        ]
    }

    fn motivational_quad(&self, subject: &NamedNode, g: &GraphName) -> Quad {
        named_quad(subject, responds_to_pred(), &self.responds_to_submission, g)
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
