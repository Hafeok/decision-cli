//! Internal helpers for `dec feedback close` (FT-033).
//!
//! Pulled out of `close.rs` to keep both files under ADR-013 Rule 1's
//! 400-line hard cap. Three responsibility clusters live here:
//!
//!   * Addressing-artifact validation (`validate_artifact_type_allowed`,
//!     `type_matches_role`, `read_artifact_metadata`).
//!   * Evidence-quad construction (`build_close_evidence`).
//!   * Actor-IRI minting and resume-check cascade rendering
//!     (`mint_actor_iri`, `actor_hash`, `run_resume_checks`,
//!     `resume_outcome_label`).

use anyhow::Result;
use chrono::Utc;
use oxigraph::model::{GraphName, NamedNode, Quad, Subject, Term};
use oxigraph::store::Store;

use crate::core::dispatch::{list_paused_groups_for_feedback, resume_check, ResumeOutcome};
use crate::core::feedback::class::FeedbackClass;
use crate::core::vocab::{addressing_artifact, closed_by, IRI_DEC_IN_STREAM};

use super::close::{CloseError, ResumedGroup};
use super::store_io::WritableStore;

/// Validate that at least one of `artifact_types` is sanctioned by the
/// routing-table allowlist for `class`. Returns a structured
/// `IneligibleAddressingArtifact` otherwise (FT-033 §Error handling).
pub(super) fn validate_artifact_type_allowed(
    class: FeedbackClass,
    artifact_types: &[String],
    class_literal: &str,
) -> Result<(), CloseError> {
    let rule = crate::core::feedback::routing::rule_for(class);
    let allowed = rule.addressing_roles;
    for t in artifact_types {
        if allowed
            .iter()
            .any(|allowed_role| type_matches_role(t, allowed_role))
        {
            return Ok(());
        }
    }
    let observed = artifact_types
        .first()
        .cloned()
        .unwrap_or_else(|| "<no rdf:type>".to_string());
    Err(CloseError::IneligibleAddressingArtifact {
        class: class_literal.to_string(),
        artifact_type: observed,
        allowed: allowed.join(", "),
    })
}

/// Decide whether `artifact_type_iri` is sanctioned by `allowed_role`.
///
/// The routing table names addressing *roles* (`spec-author`,
/// `architect`, etc.) — not RDF class IRIs — because the producing
/// role's identity is the meaningful contract. Phase A maps role to
/// type loosely: any `dec:` type whose IRI fragment matches the role
/// or its canonical produced-artifact name (e.g. `FeatureSpec`, `ADR`,
/// `CodeChange`) counts as "produced by that role".
pub(super) fn type_matches_role(artifact_type_iri: &str, allowed_role: &str) -> bool {
    let fragment = artifact_type_iri
        .rsplit_once('#')
        .map(|(_, frag)| frag)
        .or_else(|| artifact_type_iri.rsplit_once('/').map(|(_, frag)| frag))
        .unwrap_or(artifact_type_iri)
        .to_ascii_lowercase();
    match allowed_role {
        "spec-author" => {
            fragment.contains("featurespec")
                || fragment.contains("feature_spec")
                || fragment.contains("specamendment")
                || fragment.contains("specification")
        }
        "architect" => {
            fragment.contains("adr")
                || fragment.contains("architecturedecision")
                || fragment.contains("decisionrecord")
        }
        "slice-curator" => fragment.contains("slicebound") || fragment.contains("scopeamendment"),
        "verifier" => fragment.contains("verificationverdict") || fragment.contains("verdict"),
        "implementer" => fragment.contains("codechange") || fragment.contains("code_change"),
        _ => false,
    }
}

/// Construct the two evidence quads `apply_transition` attaches to the
/// `addressed → closed` mutation: `dec:addressingArtifact` and
/// `dec:closedBy`.
pub(super) fn build_close_evidence(
    fb_node: &NamedNode,
    addr_node: &NamedNode,
    closed_by_node: &NamedNode,
    g: &GraphName,
) -> Vec<Quad> {
    vec![
        Quad::new(
            fb_node.clone(),
            addressing_artifact().into_owned(),
            addr_node.clone(),
            g.clone(),
        ),
        Quad::new(
            fb_node.clone(),
            closed_by().into_owned(),
            closed_by_node.clone(),
            g.clone(),
        ),
    ]
}

/// Mint a `NamedNode` for the closing actor. If `identity` already
/// parses as an IRI we honour it; otherwise we construct a stable
/// `human-close-<ts>-<actor>-<hash>` URN so the audit trail keeps the
/// operator tag even when the CLI passes a bare username.
pub(super) fn mint_actor_iri(identity: &str, feedback_iri: &str) -> NamedNode {
    if let Ok(n) = NamedNode::new(identity) {
        return n;
    }
    let ts = Utc::now().format("%Y%m%dT%H%M%SZ");
    let safe: String = identity
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    let safe = safe.trim_matches('-');
    let safe_label = if safe.is_empty() { "anonymous" } else { safe };
    let hash = actor_hash(feedback_iri);
    let iri = format!(
        "https://decision-cli.dev/ns/session/human-close-{ts}-{safe_label}-{hash}"
    );
    // Construction is deterministic ASCII alphanumerics + `-`, so the
    // IRI is always valid; `new_unchecked` is the right call here.
    NamedNode::new_unchecked(iri)
}

fn actor_hash(feedback_iri: &str) -> String {
    use sha2::{Digest, Sha256};
    let digest = Sha256::digest(feedback_iri.as_bytes());
    digest.iter().take(6).map(|b| format!("{b:02x}")).collect()
}

/// Run resume_check on every paused dispatch that lists `fb_node` in
/// its `dec:blockedBy` set. Returns a row per group with the outcome
/// label rendered for human / JSON output.
pub(super) fn run_resume_checks(
    ws: &WritableStore,
    fb_node: &NamedNode,
) -> Result<Vec<ResumedGroup>, CloseError> {
    let groups = list_paused_groups_for_feedback(&ws.store, fb_node)
        .map_err(|e| CloseError::Other(format!("listing paused groups: {e}")))?;
    let mut out = Vec::with_capacity(groups.len());
    for group in groups {
        let outcome = resume_check(&ws.writer, &ws.store, &group)
            .map_err(|e| CloseError::Other(format!("resume_check on <{group}>: {e}")))?;
        out.push(ResumedGroup {
            group_iri: group.as_str().to_string(),
            outcome: resume_outcome_label(&outcome),
        });
    }
    Ok(out)
}

fn resume_outcome_label(o: &ResumeOutcome) -> String {
    match o {
        ResumeOutcome::Resumed => "resumed".to_string(),
        ResumeOutcome::Blocked => "blocked".to_string(),
        ResumeOutcome::StillBlocked { pending } => format!("still-blocked (pending={pending})"),
        ResumeOutcome::NotPaused { current } => format!("not-paused (current={current:?})"),
    }
}

/// Read the `(in_stream, rdf:type[])` metadata for an addressing
/// artifact. Returns `None` if no quad exists with the artifact as the
/// subject (i.e. the IRI isn't present in the store).
pub(super) fn read_artifact_metadata(
    store: &Store,
    artifact: &NamedNode,
) -> Option<(Option<String>, Vec<String>)> {
    let mut types: Vec<String> = Vec::new();
    let mut stream: Option<String> = None;
    let mut found_any = false;
    for q in store
        .quads_for_pattern(
            Some(Subject::NamedNode(artifact.clone()).as_ref()),
            None,
            None,
            None,
        )
        .filter_map(Result::ok)
    {
        found_any = true;
        let p = q.predicate.as_str();
        if p == "http://www.w3.org/1999/02/22-rdf-syntax-ns#type" {
            if let Term::NamedNode(n) = &q.object {
                types.push(n.as_str().to_string());
            }
        } else if p == IRI_DEC_IN_STREAM {
            if let Term::NamedNode(n) = &q.object {
                stream = Some(n.as_str().to_string());
            }
        }
    }
    if !found_any {
        None
    } else {
        Some((stream, types))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn type_matches_role_recognises_known_roles() {
        assert!(type_matches_role(
            "https://decision-cli.dev/ns#FeatureSpec",
            "spec-author"
        ));
        assert!(type_matches_role("urn:adr:ADR-099", "architect"));
        assert!(type_matches_role(
            "https://decision-cli.dev/ns#CodeChange",
            "implementer"
        ));
        assert!(!type_matches_role(
            "https://decision-cli.dev/ns#FeatureSpec",
            "architect"
        ));
    }

    #[test]
    fn ineligible_artifact_returns_structured_error() {
        let err = validate_artifact_type_allowed(
            FeedbackClass::Gap,
            &["https://decision-cli.dev/ns#CodeChange".to_string()],
            "gap",
        )
        .unwrap_err();
        match err {
            CloseError::IneligibleAddressingArtifact {
                class,
                artifact_type,
                allowed,
            } => {
                assert_eq!(class, "gap");
                assert!(artifact_type.contains("CodeChange"));
                assert!(allowed.contains("spec-author"));
            }
            other => panic!("expected IneligibleAddressingArtifact, got {other:?}"),
        }
    }

    #[test]
    fn allowed_artifact_passes_validation() {
        validate_artifact_type_allowed(
            FeedbackClass::Gap,
            &["https://decision-cli.dev/ns#FeatureSpec".to_string()],
            "gap",
        )
        .expect("gap → feature_spec amendment is allowed");
    }

    #[test]
    fn mint_actor_iri_round_trips_existing_iris() {
        let iri = mint_actor_iri("https://example.org/me", "urn:f:1");
        assert_eq!(iri.as_str(), "https://example.org/me");
    }

    #[test]
    fn mint_actor_iri_constructs_synthetic_for_bare_names() {
        let iri = mint_actor_iri("alice", "urn:f:1");
        assert!(iri
            .as_str()
            .starts_with("https://decision-cli.dev/ns/session/human-close-"));
        assert!(iri.as_str().contains("-alice-"));
    }
}
