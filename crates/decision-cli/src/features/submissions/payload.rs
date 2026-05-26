//! JSON request shape for `POST /submissions` (FT-094).
//!
//! Wire format mirrors the `WorkerImageSubmission` artifact (FT-087)
//! exactly, with a few quality-of-life conveniences for producer CI:
//!
//! - `id` is optional. When omitted, the handler derives a `UUIDv4` id so
//!   first-time CI authors don't need to mint one client-side.
//! - `external_origin` is optional. When omitted, the handler derives a
//!   stable string from `claimed_build_run_url` (the CI run identifier)
//!   so the `BoundaryArtifact` escape-hatch invariant still holds.
//! - `lifecycle_state` is implicitly `received`; clients cannot post a
//!   Submission in any other state — the `WorkerCurator` owns transitions.
//!
//! See `crates/decision-cli/src/core/ontology/worker_image_submission/`
//! for the canonical SHACL shape this payload must satisfy.

use serde::{Deserialize, Serialize};

use crate::core::ontology::worker_image_submission::{
    SubmissionLifecycleState, WorkerImageSubmission,
};
use oxigraph::model::NamedNode;

/// JSON payload accepted by `POST /submissions`. Field names mirror the
/// graph predicates (`claimed_capability_tag`, `claimed_source_repo_uri`,
/// etc.) so the wire / graph shape line up 1:1.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubmissionPayload {
    /// Optional caller-supplied id. `UUIDv4` when omitted.
    #[serde(default)]
    pub id: Option<String>,
    /// Proposed OCI reference pinned by `@sha256:` digest.
    pub candidate_registry_ref: String,
    /// Capability-tag claims (one or more).
    pub claimed_capability_tags: Vec<String>,
    /// `dec:Role` IRIs the candidate image claims compatibility with.
    /// Empty vector is permitted in slice 1 (Curator decides bindings).
    #[serde(default)]
    pub claimed_compatible_roles: Vec<String>,
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
    /// Optional `BoundaryArtifact` `dec:external_origin` literal. Defaults
    /// to the CI run URL when omitted.
    #[serde(default)]
    pub external_origin: Option<String>,
}

impl SubmissionPayload {
    /// Materialise the payload into a [`WorkerImageSubmission`]. Fills
    /// in the optional fields with deterministic defaults so the
    /// downstream SHACL validators see a fully-populated artifact.
    ///
    /// Returns `Err` if a declared `claimed_compatible_roles` entry is
    /// not a valid IRI. Other invariants (digest pin, capability-tag
    /// cardinality, etc.) are deferred to the SHACL layer so the
    /// validator reports a structured 422 response body for the client.
    pub fn into_submission(self, id_default: impl FnOnce() -> String) -> Result<WorkerImageSubmission, BadRoleIri> {
        let id = self.id.unwrap_or_else(id_default);
        let external_origin = self
            .external_origin
            .unwrap_or_else(|| self.claimed_build_run_url.clone());
        let mut roles: Vec<NamedNode> = Vec::with_capacity(self.claimed_compatible_roles.len());
        for raw in &self.claimed_compatible_roles {
            match NamedNode::new(raw) {
                Ok(n) => roles.push(n),
                Err(err) => {
                    return Err(BadRoleIri {
                        offender: raw.clone(),
                        detail: err.to_string(),
                    });
                }
            }
        }
        Ok(WorkerImageSubmission {
            id,
            candidate_registry_ref: self.candidate_registry_ref,
            claimed_capability_tags: self.claimed_capability_tags,
            claimed_compatible_roles: roles,
            claimed_sbom_ref: self.claimed_sbom_ref,
            claimed_signature_subject: self.claimed_signature_subject,
            claimed_signature_issuer: self.claimed_signature_issuer,
            claimed_source_repo_uri: self.claimed_source_repo_uri,
            claimed_source_commit_hash: self.claimed_source_commit_hash,
            claimed_build_run_url: self.claimed_build_run_url,
            lifecycle_state: SubmissionLifecycleState::Received,
            external_origin,
            produced_workerimage: None,
            produced_feedback: None,
        })
    }
}

/// Surfaced when a `claimed_compatible_roles` entry fails to parse as
/// a `NamedNode`. Mapped to 422 by the handler.
#[derive(Debug, Clone)]
pub struct BadRoleIri {
    /// The raw string that failed to parse.
    pub offender: String,
    /// The parser's own error detail.
    pub detail: String,
}

#[cfg(test)]
mod payload_unit_tests {
    use super::*;

    fn baseline_payload() -> SubmissionPayload {
        SubmissionPayload {
            id: Some("sub-1".to_string()),
            candidate_registry_ref: "ghcr.io/example/worker@sha256:deadbeef".to_string(),
            claimed_capability_tags: vec!["code-writer".to_string()],
            claimed_compatible_roles: vec![],
            claimed_sbom_ref: "ghcr.io/example/worker@sha256:cafebabe".to_string(),
            claimed_signature_subject: "https://github.com/example/.github/workflows/build.yml@refs/heads/main".to_string(),
            claimed_signature_issuer: "https://token.actions.githubusercontent.com".to_string(),
            claimed_source_repo_uri: "https://github.com/example/worker".to_string(),
            claimed_source_commit_hash: "abc123".to_string(),
            claimed_build_run_url: "https://github.com/example/worker/actions/runs/1".to_string(),
            external_origin: Some("github-actions:example/worker/runs/1".to_string()),
        }
    }

    #[test]
    fn into_submission_preserves_fields() {
        let payload = baseline_payload();
        let sub = payload
            .clone()
            .into_submission(|| "fallback".to_string())
            .expect("conversion");
        assert_eq!(sub.id, "sub-1");
        assert_eq!(sub.candidate_registry_ref, payload.candidate_registry_ref);
        assert_eq!(sub.claimed_capability_tags, payload.claimed_capability_tags);
        assert_eq!(sub.external_origin, "github-actions:example/worker/runs/1");
        assert_eq!(sub.lifecycle_state, SubmissionLifecycleState::Received);
    }

    #[test]
    fn into_submission_defaults_id_when_missing() {
        let mut payload = baseline_payload();
        payload.id = None;
        let sub = payload
            .into_submission(|| "minted-id".to_string())
            .expect("conversion");
        assert_eq!(sub.id, "minted-id");
    }

    #[test]
    fn into_submission_defaults_external_origin_to_run_url() {
        let mut payload = baseline_payload();
        payload.external_origin = None;
        let sub = payload
            .into_submission(|| "x".to_string())
            .expect("conversion");
        assert_eq!(sub.external_origin, sub.claimed_build_run_url);
    }

    #[test]
    fn into_submission_rejects_bad_role_iri() {
        let mut payload = baseline_payload();
        payload.claimed_compatible_roles = vec!["not a valid iri".to_string()];
        let err = payload
            .into_submission(|| "x".to_string())
            .expect_err("bad iri must fail");
        assert_eq!(err.offender, "not a valid iri");
    }
}
