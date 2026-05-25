//! In-memory `dec:WorkerImage` shape + RDF serialisation (FT-086 / ADR-055).

use oxigraph::model::{GraphName, Literal, NamedNode, NamedNodeRef, Quad};

use crate::core::vocab::{
    build_run_url_pred, capability_tag_pred, compatible_role_pred, conformance_audit_pred,
    eligibility_status_pred, registry_ref_pred, sbom_ref_pred, signed_by_issuer_pred,
    signed_by_subject_pred, source_commit_hash_pred, source_repo_uri_pred, worker_image_class,
    worker_image_id_pred, worker_image_name_pred, worker_image_version_pred,
    ELIGIBILITY_CANDIDATE, ELIGIBILITY_DEPRECATED, ELIGIBILITY_PULLED, ELIGIBILITY_QUALIFIED,
    IRI_DEC_WORKER_IMAGE_PREFIX,
};

pub(super) const RDF_TYPE: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#type";

/// Lifecycle status of a `dec:WorkerImage` (ADR-055).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EligibilityStatus {
    /// Qualified — eligible for dispatch.
    Qualified,
    /// Candidate — registered but not yet bound to any capability tag.
    Candidate,
    /// Deprecated — bound elsewhere but discouraged for new bindings.
    Deprecated,
    /// Pulled — refuses dispatch (e.g. CVE in image, audit failure).
    Pulled,
}

impl EligibilityStatus {
    /// Stable wire string for the status enum.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Qualified => ELIGIBILITY_QUALIFIED,
            Self::Candidate => ELIGIBILITY_CANDIDATE,
            Self::Deprecated => ELIGIBILITY_DEPRECATED,
            Self::Pulled => ELIGIBILITY_PULLED,
        }
    }

    /// Parse a status from its wire string. Returns `None` for unknown values.
    #[must_use]
    pub fn try_from_str(s: &str) -> Option<Self> {
        match s {
            ELIGIBILITY_QUALIFIED => Some(Self::Qualified),
            ELIGIBILITY_CANDIDATE => Some(Self::Candidate),
            ELIGIBILITY_DEPRECATED => Some(Self::Deprecated),
            ELIGIBILITY_PULLED => Some(Self::Pulled),
            _ => None,
        }
    }
}

/// In-memory `dec:WorkerImage` artifact (FT-086).
///
/// Identity is `(id, version)` and the canonical IRI is
/// `https://decision-cli.dev/ns/worker-image/<id>/v<version>`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkerImage {
    /// Stable id used for catalog lookup.
    pub id: String,
    /// Human-readable display name.
    pub name: String,
    /// Semver version string (e.g. `"1.2.0"`).
    pub version: String,
    /// OCI reference with digest, e.g. `ghcr.io/example/worker@sha256:abc…`.
    pub registry_ref: String,
    /// Capability-tag claims (multi-valued; at least one).
    pub capability_tags: Vec<String>,
    /// `dec:Role` IRIs this image can serve (multi-valued, optional).
    pub compatible_roles: Vec<NamedNode>,
    /// Sigstore Fulcio cert subject.
    pub signed_by_subject: String,
    /// Sigstore Fulcio issuer URI.
    pub signed_by_issuer: String,
    /// OCI referrer URI for the SBOM attestation.
    pub sbom_ref: String,
    /// IRIs of `dec:ConformanceAudit` artifacts referencing this image.
    pub conformance_audits: Vec<NamedNode>,
    /// Lifecycle status.
    pub eligibility_status: EligibilityStatus,
    /// Provenance: source repository URI.
    pub source_repo_uri: String,
    /// Provenance: commit SHA the image was built from.
    pub source_commit_hash: String,
    /// Provenance: CI run URL.
    pub build_run_url: String,
}

impl WorkerImage {
    /// Construct the canonical IRI for this image:
    /// `https://decision-cli.dev/ns/worker-image/<id>/v<version>`.
    #[must_use]
    pub fn iri(&self) -> NamedNode {
        NamedNode::new_unchecked(format!(
            "{prefix}{id}/v{version}",
            prefix = IRI_DEC_WORKER_IMAGE_PREFIX,
            id = self.id,
            version = self.version,
        ))
    }

    /// Serialise the worker image to RDF quads in the supplied named graph.
    #[must_use]
    pub fn to_quads(&self, graph: NamedNodeRef<'_>) -> Vec<Quad> {
        let g: GraphName = graph.into_owned().into();
        let subject = self.iri();
        let mut quads = self.header_quads(&subject, &g);
        quads.extend(self.string_quads(&subject, &g));
        quads.extend(self.tag_quads(&subject, &g));
        quads.extend(self.role_quads(&subject, &g));
        quads.extend(self.audit_quads(&subject, &g));
        quads
    }

    fn header_quads(&self, subject: &NamedNode, g: &GraphName) -> Vec<Quad> {
        let rdf_type = NamedNodeRef::new_unchecked(RDF_TYPE);
        vec![Quad::new(
            subject.clone(),
            rdf_type,
            worker_image_class(),
            g.clone(),
        )]
    }

    fn string_quads(&self, subject: &NamedNode, g: &GraphName) -> Vec<Quad> {
        vec![
            literal_quad(subject, worker_image_id_pred(), &self.id, g),
            literal_quad(subject, worker_image_name_pred(), &self.name, g),
            literal_quad(subject, worker_image_version_pred(), &self.version, g),
            literal_quad(subject, registry_ref_pred(), &self.registry_ref, g),
            literal_quad(
                subject,
                eligibility_status_pred(),
                self.eligibility_status.as_str(),
                g,
            ),
            literal_quad(
                subject,
                signed_by_subject_pred(),
                &self.signed_by_subject,
                g,
            ),
            literal_quad(subject, signed_by_issuer_pred(), &self.signed_by_issuer, g),
            literal_quad(subject, sbom_ref_pred(), &self.sbom_ref, g),
            literal_quad(subject, source_repo_uri_pred(), &self.source_repo_uri, g),
            literal_quad(
                subject,
                source_commit_hash_pred(),
                &self.source_commit_hash,
                g,
            ),
            literal_quad(subject, build_run_url_pred(), &self.build_run_url, g),
        ]
    }

    fn tag_quads(&self, subject: &NamedNode, g: &GraphName) -> Vec<Quad> {
        self.capability_tags
            .iter()
            .map(|tag| literal_quad(subject, capability_tag_pred(), tag, g))
            .collect()
    }

    fn role_quads(&self, subject: &NamedNode, g: &GraphName) -> Vec<Quad> {
        self.compatible_roles
            .iter()
            .map(|role| named_quad(subject, compatible_role_pred(), role, g))
            .collect()
    }

    fn audit_quads(&self, subject: &NamedNode, g: &GraphName) -> Vec<Quad> {
        self.conformance_audits
            .iter()
            .map(|a| named_quad(subject, conformance_audit_pred(), a, g))
            .collect()
    }
}

pub(super) fn literal_quad(
    s: &NamedNode,
    p: NamedNodeRef<'_>,
    value: &str,
    g: &GraphName,
) -> Quad {
    Quad::new(
        s.clone(),
        p.into_owned(),
        Literal::new_simple_literal(value),
        g.clone(),
    )
}

pub(super) fn named_quad(
    s: &NamedNode,
    p: NamedNodeRef<'_>,
    o: &NamedNode,
    g: &GraphName,
) -> Quad {
    Quad::new(s.clone(), p.into_owned(), o.clone(), g.clone())
}
