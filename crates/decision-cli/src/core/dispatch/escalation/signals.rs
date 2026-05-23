//! Signal collection from worker attempts (FT-062 §Signal collection).

use crate::core::bundle::Bundle;
use crate::core::feedback::FeedbackClass;

use super::types::{DispatchAttempt, SignalSet};

/// Map an attempt's structured outputs into the closed-vocabulary
/// signal set the trigger evaluator reads.
///
/// `prior_attempts` is the 1-indexed position of the attempt in the
/// chain — the `prior_attempts_ge_N` triggers fire when this value
/// reaches `N`.
#[must_use]
pub fn collect_signals(
    bundle: &Bundle,
    attempt: &DispatchAttempt,
    prior_attempts: u32,
) -> SignalSet {
    let audit_pass = attempt.audit_outcome.as_ref().map(|a| a.passes);
    let mut feedback_classes: Vec<FeedbackClass> = Vec::new();
    let mut feedback_critical = false;
    for f in &attempt.feedback {
        if !feedback_classes.iter().any(|c| *c == f.class) {
            feedback_classes.push(f.class);
        }
        if f.critical {
            feedback_critical = true;
        }
    }
    SignalSet {
        stakes: bundle.stakes,
        audit_pass,
        feedback_classes,
        feedback_critical,
        confidence: attempt.result.confidence(),
        prior_attempts,
        verdict: attempt.result.verdict(),
    }
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

    use super::super::types::{DispatchAttempt, FeedbackArtifact, WorkerResult};

    fn focal() -> NamedNode {
        NamedNode::new_unchecked("https://example.com/focal")
    }

    fn routine_bundle() -> Bundle {
        Bundle {
            hash: "abc123".to_string(),
            focal: focal(),
            stakes: Stakes::Routine,
        }
    }

    fn cap(id: &str) -> ResolvedCapability {
        ResolvedCapability {
            capability_id: id.to_string(),
            capability_version: 1,
            endpoint: Endpoint::Scaleway,
            model_identifier: format!("model-of-{id}"),
            max_output: 1024,
            supports_tool_calling: true,
            configurable_effort: false,
            binding_version: 1,
        }
    }

    fn attempt(result: WorkerResult, feedback: Vec<FeedbackArtifact>) -> DispatchAttempt {
        DispatchAttempt {
            session_id: NamedNode::new_unchecked("https://decision-cli.dev/ns/session/s1"),
            capability: cap("code-writer"),
            result,
            feedback,
            audit_outcome: None,
        }
    }

    #[test]
    fn extracts_classes_and_critical_dedup() {
        let b = routine_bundle();
        let a = attempt(
            WorkerResult::Verdict {
                kind: Verdict::AmendmentRequired,
                confidence: Some(0.4),
            },
            vec![
                FeedbackArtifact {
                    class: FeedbackClass::Gap,
                    critical: false,
                },
                FeedbackArtifact {
                    class: FeedbackClass::Unimplementable,
                    critical: true,
                },
                FeedbackArtifact {
                    class: FeedbackClass::Gap,
                    critical: false,
                },
            ],
        );
        let s = collect_signals(&b, &a, 1);
        assert_eq!(s.feedback_classes.len(), 2);
        assert!(s.feedback_critical);
        assert_eq!(s.confidence, Some(0.4));
        assert_eq!(s.prior_attempts, 1);
    }

    #[test]
    fn audit_pass_propagated() {
        use super::super::types::AuditOutcome;
        let b = routine_bundle();
        let mut a = attempt(WorkerResult::Failed, vec![]);
        a.audit_outcome = Some(AuditOutcome { passes: false });
        let s = collect_signals(&b, &a, 2);
        assert_eq!(s.audit_pass, Some(false));
        assert_eq!(s.prior_attempts, 2);
        assert!(s.confidence.is_none());
        assert!(s.verdict.is_none());
    }
}
