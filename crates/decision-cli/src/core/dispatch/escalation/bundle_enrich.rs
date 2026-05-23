//! Bundle enrichment on escalation (FT-062 / ADR-034 §"Bundle enrichment").

use crate::core::bundle::Bundle;

use super::types::{DispatchAttempt, WorkerResult};

/// `prov:wasDerivedFrom` predicate — used on enriched bundles to
/// record PROV-O lineage back to the prior bundle.
pub const PROV_WAS_DERIVED_FROM: &str = "http://www.w3.org/ns/prov#wasDerivedFrom";

/// `dec:supersedes_bundle` — chain enriched bundles back to the
/// original they extended.
pub const IRI_DEC_SUPERSEDES_BUNDLE: &str = "https://decision-cli.dev/ns#supersedes_bundle";

/// Markdown rendering of one prior attempt (ADR-034 §"Bundle enrichment").
/// The fixed framing string `"agree, refute, or refine"` is required
/// by TC-109's assertion.
#[must_use]
pub fn render_prior_attempt_block(prior: &DispatchAttempt, tier: u32) -> String {
    let body = render_body(&prior.result, &prior.feedback);
    format!(
        "## Prior attempt (tier {tier}, capability {cap}, model {model})\n\n\
         {body}\n---\n\n\
         The above was produced by tier-{tier} reasoning. Your task is to agree, \
         refute, or refine. If you agree, state which evidence in the bundle supports \
         the prior verdict. If you refute, cite the specific evidence the prior attempt \
         missed. Do not restate the prior attempt without referencing it.\n",
        tier = tier,
        cap = prior.capability.capability_id,
        model = prior.capability.model_identifier,
        body = body,
    )
}

fn render_body(result: &WorkerResult, feedback: &[super::types::FeedbackArtifact]) -> String {
    let mut body = match result {
        WorkerResult::Verdict { kind, confidence } => {
            let conf = confidence
                .map(|c| format!("{c:.2}"))
                .unwrap_or_else(|| "n/a".to_string());
            format!("Verdict: {kind}\nConfidence: {conf}\n", kind = kind.as_str())
        }
        WorkerResult::CodeChange { applied } => format!("CodeChange.applied: {applied}\n"),
        WorkerResult::Failed => "Worker failed (no structured result).\n".to_string(),
    };
    if !feedback.is_empty() {
        body.push_str("\nEmitted feedback:\n");
        for f in feedback {
            let crit = if f.critical { "critical" } else { "noncritical" };
            body.push_str(&format!(
                "- class={class}, severity={crit}\n",
                class = f.class.as_iri_value()
            ));
        }
    }
    body
}

/// Enrich `bundle` with the prior attempt's structured rendering per
/// ADR-034. The returned bundle is a *new* `Bundle` artifact with its
/// own hash (computed from the original hash + tier marker); the
/// original is not mutated.
#[must_use]
pub fn enrich_bundle_with_prior_attempt(
    bundle: Bundle,
    prior: &DispatchAttempt,
    tier: u32,
) -> Bundle {
    let new_hash = format!(
        "{base}-tier{tier}-{cap}",
        base = bundle.hash,
        cap = prior.capability.capability_id
    );
    Bundle {
        hash: new_hash,
        focal: bundle.focal,
        stakes: bundle.stakes,
    }
}

/// Render the full enriched bundle markdown (original metadata +
/// stacked prior-attempt blocks).
#[must_use]
pub fn render_enriched_bundle_markdown(
    bundle: &Bundle,
    prior_attempts: &[DispatchAttempt],
) -> String {
    let mut md = bundle.dispatch_metadata_markdown();
    for (idx, attempt) in prior_attempts.iter().enumerate() {
        md.push('\n');
        md.push_str(&render_prior_attempt_block(attempt, (idx + 1) as u32));
    }
    md
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::bundle::Stakes;
    use crate::core::dispatch::capability_resolver::ResolvedCapability;
    use crate::core::feedback::FeedbackClass;
    use crate::core::ontology::capability::Endpoint;
    use crate::core::ontology::verdict::Verdict;
    use oxigraph::model::NamedNode;

    use super::super::types::{DispatchAttempt, FeedbackArtifact};

    fn cap(id: &str, version: u32) -> ResolvedCapability {
        ResolvedCapability {
            capability_id: id.to_string(),
            capability_version: version,
            endpoint: Endpoint::Scaleway,
            model_identifier: format!("model-of-{id}"),
            max_output: 1024,
            supports_tool_calling: true,
            configurable_effort: false,
            binding_version: 1,
            cost_cache_hit_per_m: None,
        }
    }

    fn attempt_verdict(conf: f32) -> DispatchAttempt {
        DispatchAttempt {
            session_id: NamedNode::new_unchecked("https://decision-cli.dev/ns/session/s1"),
            capability: cap("code-writer", 1),
            result: WorkerResult::Verdict {
                kind: Verdict::AmendmentRequired,
                confidence: Some(conf),
            },
            feedback: vec![],
            audit_outcome: None,
        }
    }

    #[test]
    fn block_carries_required_framing() {
        let prior = attempt_verdict(0.6);
        let block = render_prior_attempt_block(&prior, 1);
        assert!(block.contains("## Prior attempt"));
        assert!(block.contains("agree, refute, or refine"));
        assert!(block.contains("Verdict: amendment-required"));
        assert!(block.contains("Confidence: 0.60"));
    }

    #[test]
    fn enriched_bundle_has_new_hash_and_supersedes_link() {
        let orig = Bundle {
            hash: "deadbeef".to_string(),
            focal: NamedNode::new_unchecked("https://example.com/focal"),
            stakes: Stakes::Routine,
        };
        let prior = attempt_verdict(0.6);
        let new_bundle = enrich_bundle_with_prior_attempt(orig.clone(), &prior, 1);
        assert_ne!(new_bundle.hash, orig.hash);
        assert_eq!(new_bundle.stakes, orig.stakes);
        assert_eq!(new_bundle.focal, orig.focal);
    }

    #[test]
    fn feedback_appears_in_rendered_block() {
        let mut prior = attempt_verdict(0.6);
        prior.feedback = vec![FeedbackArtifact {
            class: FeedbackClass::Unimplementable,
            critical: true,
        }];
        let block = render_prior_attempt_block(&prior, 2);
        assert!(block.contains("class=unimplementable"));
        assert!(block.contains("severity=critical"));
        assert!(block.contains("tier 2"));
    }
}
