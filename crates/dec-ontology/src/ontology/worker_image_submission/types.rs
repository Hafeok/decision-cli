//! In-memory `dec:WorkerImageSubmission` shape + RDF serialisation (FT-087 / ADR-040 / ADR-055).

use oxrdf::{GraphName, Literal, NamedNode, NamedNodeRef, Quad};

use crate::ontology::boundary_artifact::{external_origin_pred, initial_request_class};
use crate::vocab::{
    candidate_registry_ref_pred, claimed_build_run_url_pred, claimed_capability_tag_pred,
    claimed_compatible_role_pred, claimed_sbom_ref_pred, claimed_signature_issuer_pred,
    claimed_signature_subject_pred, claimed_source_commit_hash_pred, claimed_source_repo_uri_pred,
    produced_feedback_pred, produced_workerimage_pred, submission_id_pred,
    submission_lifecycle_state_pred, worker_image_submission_class,
    IRI_DEC_WORKER_IMAGE_SUBMISSION, IRI_DEC_WORKER_IMAGE_SUBMISSION_PREFIX,
    SUBMISSION_STATE_ADMITTED, SUBMISSION_STATE_RECEIVED, SUBMISSION_STATE_REJECTED,
    SUBMISSION_STATE_UNDER_REVIEW,
};

/// IRI of `rdf:type`.
pub const RDF_TYPE: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#type";

/// IRI of the `dec:WorkerImageSubmission` class.
pub const WORKER_IMAGE_SUBMISSION_CLASS_IRI: &str = IRI_DEC_WORKER_IMAGE_SUBMISSION;

/// Lifecycle state of a `dec:WorkerImageSubmission` (FT-087 §Scope).
///
/// Transitions are reachable only via Curator session output (FT-092).
/// Manual state edits are rejected by the Curator workflow, not by this
/// type — this struct only ensures the persisted literal is one of the
/// declared values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SubmissionLifecycleState {
    /// CI posted the Submission; awaiting Curator review.
    Received,
    /// Curator session has started consuming the Submission.
    UnderReview,
    /// Curator admitted the Submission and minted a `dec:WorkerImage`.
    Admitted,
    /// Curator rejected the Submission and produced a `dec:Feedback`.
    Rejected,
}

impl SubmissionLifecycleState {
    /// Stable wire string for the lifecycle enum.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Received => SUBMISSION_STATE_RECEIVED,
            Self::UnderReview => SUBMISSION_STATE_UNDER_REVIEW,
            Self::Admitted => SUBMISSION_STATE_ADMITTED,
            Self::Rejected => SUBMISSION_STATE_REJECTED,
        }
    }

    /// Parse a state from its wire string. Returns `None` for unknown values.
    #[must_use]
    pub fn try_from_str(s: &str) -> Option<Self> {
        match s {
            SUBMISSION_STATE_RECEIVED => Some(Self::Received),
            SUBMISSION_STATE_UNDER_REVIEW => Some(Self::UnderReview),
            SUBMISSION_STATE_ADMITTED => Some(Self::Admitted),
            SUBMISSION_STATE_REJECTED => Some(Self::Rejected),
            _ => None,
        }
    }
}

/// In-memory `dec:WorkerImageSubmission` artifact (FT-087).
///
/// Identity is `id`. The canonical IRI is
/// `https://decision-cli.dev/ns/worker-image-submission/<id>`.
///
/// Always classified as a `dec:InitialRequest` (a `dec:BoundaryArtifact`
/// subclass per FT-071 / ADR-040), so consumers of the dual-provenance
/// discipline (ADR-038) see the boundary class membership at write time
/// and exempt the Submission from motivational provenance enforcement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkerImageSubmission {
    /// Stable id used for catalog lookup.
    pub id: String,
    /// Proposed OCI reference with digest, e.g. `ghcr.io/example/worker@sha256:abc…`.
    pub candidate_registry_ref: String,
    /// Capability-tag claims (multi-valued; at least one).
    pub claimed_capability_tags: Vec<String>,
    /// `dec:Role` IRIs the candidate image claims compatibility with.
    pub claimed_compatible_roles: Vec<NamedNode>,
    /// OCI referrer URI for the SBOM attestation.
    pub claimed_sbom_ref: String,
    /// Sigstore Fulcio certificate subject.
    pub claimed_signature_subject: String,
    /// Sigstore Fulcio issuer URI.
    pub claimed_signature_issuer: String,
    /// Provenance: source repository URI.
    pub claimed_source_repo_uri: String,
    /// Provenance: commit SHA the candidate image was built from.
    pub claimed_source_commit_hash: String,
    /// Provenance: CI run URL.
    pub claimed_build_run_url: String,
    /// Submission lifecycle state. Defaults to `Received` at construction.
    pub lifecycle_state: SubmissionLifecycleState,
    /// `dec:external_origin` literal documenting how the Submission entered
    /// the system (per FT-071 / ADR-040). Typically the CI run identifier
    /// or producer-repo reference.
    pub external_origin: String,
    /// Edge written on admission: the minted WorkerImage IRI.
    pub produced_workerimage: Option<NamedNode>,
    /// Edge written on rejection: the Feedback IRI.
    pub produced_feedback: Option<NamedNode>,
}

impl WorkerImageSubmission {
    /// Construct the canonical IRI for this Submission:
    /// `https://decision-cli.dev/ns/worker-image-submission/<id>`.
    #[must_use]
    pub fn iri(&self) -> NamedNode {
        NamedNode::new_unchecked(format!(
            "{prefix}{id}",
            prefix = IRI_DEC_WORKER_IMAGE_SUBMISSION_PREFIX,
            id = self.id,
        ))
    }

    /// Serialise the Submission to RDF quads in the supplied named graph.
    /// Always emits two `rdf:type` triples — `dec:WorkerImageSubmission`
    /// AND `dec:InitialRequest` — so SHACL `sh:class dec:BoundaryArtifact`
    /// reasoning admits the Submission as a BoundaryArtifact via the
    /// subClassOf chain declared in the embedded ontology.
    #[must_use]
    pub fn to_quads(&self, graph: NamedNodeRef<'_>) -> Vec<Quad> {
        let g: GraphName = graph.into_owned().into();
        let subject = self.iri();
        let mut quads = self.header_quads(&subject, &g);
        quads.extend(self.string_quads(&subject, &g));
        quads.extend(self.tag_quads(&subject, &g));
        quads.extend(self.role_quads(&subject, &g));
        quads.extend(self.edge_quads(&subject, &g));
        quads
    }

    fn header_quads(&self, subject: &NamedNode, g: &GraphName) -> Vec<Quad> {
        let rdf_type = NamedNodeRef::new_unchecked(RDF_TYPE);
        vec![
            Quad::new(
                subject.clone(),
                rdf_type,
                worker_image_submission_class(),
                g.clone(),
            ),
            // Co-declared as :InitialRequest so SHACL `sh:class
            // dec:BoundaryArtifact` is satisfied via the subClassOf chain.
            Quad::new(
                subject.clone(),
                rdf_type,
                initial_request_class(),
                g.clone(),
            ),
        ]
    }

    fn string_quads(&self, subject: &NamedNode, g: &GraphName) -> Vec<Quad> {
        let mut out = Vec::with_capacity(10);
        for (pred, value) in self.literal_pred_value_pairs() {
            out.push(literal_quad(subject, pred, value, g));
        }
        out.push(literal_quad(
            subject,
            submission_lifecycle_state_pred(),
            self.lifecycle_state.as_str(),
            g,
        ));
        out.push(xsd_string_quad(
            subject,
            external_origin_pred(),
            &self.external_origin,
            g,
        ));
        out
    }

    fn literal_pred_value_pairs(&self) -> [(NamedNodeRef<'static>, &str); 8] {
        [
            (submission_id_pred(), self.id.as_str()),
            (
                candidate_registry_ref_pred(),
                self.candidate_registry_ref.as_str(),
            ),
            (claimed_sbom_ref_pred(), self.claimed_sbom_ref.as_str()),
            (
                claimed_signature_subject_pred(),
                self.claimed_signature_subject.as_str(),
            ),
            (
                claimed_signature_issuer_pred(),
                self.claimed_signature_issuer.as_str(),
            ),
            (
                claimed_source_repo_uri_pred(),
                self.claimed_source_repo_uri.as_str(),
            ),
            (
                claimed_source_commit_hash_pred(),
                self.claimed_source_commit_hash.as_str(),
            ),
            (
                claimed_build_run_url_pred(),
                self.claimed_build_run_url.as_str(),
            ),
        ]
    }

    fn tag_quads(&self, subject: &NamedNode, g: &GraphName) -> Vec<Quad> {
        self.claimed_capability_tags
            .iter()
            .map(|t| literal_quad(subject, claimed_capability_tag_pred(), t, g))
            .collect()
    }

    fn role_quads(&self, subject: &NamedNode, g: &GraphName) -> Vec<Quad> {
        self.claimed_compatible_roles
            .iter()
            .map(|role| named_quad(subject, claimed_compatible_role_pred(), role, g))
            .collect()
    }

    fn edge_quads(&self, subject: &NamedNode, g: &GraphName) -> Vec<Quad> {
        let mut out = Vec::new();
        if let Some(w) = &self.produced_workerimage {
            out.push(named_quad(subject, produced_workerimage_pred(), w, g));
        }
        if let Some(f) = &self.produced_feedback {
            out.push(named_quad(subject, produced_feedback_pred(), f, g));
        }
        out
    }
}

/// Build a plain-literal quad `(s, p, "value", g)`.
pub fn literal_quad(s: &NamedNode, p: NamedNodeRef<'_>, value: &str, g: &GraphName) -> Quad {
    Quad::new(
        s.clone(),
        p.into_owned(),
        Literal::new_simple_literal(value),
        g.clone(),
    )
}

/// Build an `xsd:string`-typed literal quad `(s, p, "value", g)`.
pub fn xsd_string_quad(s: &NamedNode, p: NamedNodeRef<'_>, value: &str, g: &GraphName) -> Quad {
    Quad::new(
        s.clone(),
        p.into_owned(),
        Literal::new_typed_literal(
            value,
            NamedNode::new_unchecked("http://www.w3.org/2001/XMLSchema#string"),
        ),
        g.clone(),
    )
}

/// Build an IRI-object quad `(s, p, o, g)`.
pub fn named_quad(s: &NamedNode, p: NamedNodeRef<'_>, o: &NamedNode, g: &GraphName) -> Quad {
    Quad::new(s.clone(), p.into_owned(), o.clone(), g.clone())
}
