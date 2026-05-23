//! Trigger evaluation with step selection (FT-062 §Trigger evaluation).

use oxigraph::model::NamedNode;

use crate::core::bundle::Stakes;
use crate::core::dispatch::capability_resolver::ResolvedCapability;
use crate::core::feedback::FeedbackClass;
use crate::core::ontology::role_binding::{EscalationStep, RoleBinding, TriggerSignal};

use super::types::SignalSet;

/// Canonical capability IRI for `cap` (matches `cap_iri(id, version)`).
#[must_use]
pub fn capability_iri(cap: &ResolvedCapability) -> NamedNode {
    NamedNode::new_unchecked(format!(
        "https://decision-cli.dev/ns/capability/{id}/v{version}",
        id = cap.capability_id,
        version = cap.capability_version,
    ))
}

/// Walk the binding's `escalation_steps` in order, return the first
/// step whose triggers match the signal set and whose target
/// capability differs from the current one.
///
/// Returns `None` when no remaining step matches — the dispatcher
/// treats this as the success-path terminator.
#[must_use]
pub fn find_next_escalation_step(
    binding: &RoleBinding,
    current_capability: &ResolvedCapability,
    signals: &SignalSet,
) -> Option<EscalationStep> {
    let current_iri = capability_iri(current_capability);
    for step in &binding.escalation_steps {
        if step.step_capability.as_str() == current_iri.as_str() {
            // No self-escalation.
            continue;
        }
        if !step.triggers.iter().any(|t| evaluate_trigger(*t, signals)) {
            continue;
        }
        return Some(step.clone());
    }
    None
}

/// Pure evaluator for one trigger against one signal set. Total over
/// the closed vocabulary (per ADR-034).
#[must_use]
pub fn evaluate_trigger(trigger: TriggerSignal, signals: &SignalSet) -> bool {
    match trigger {
        TriggerSignal::StakesRoutine => signals.stakes == Stakes::Routine,
        TriggerSignal::StakesElevated => signals.stakes == Stakes::Elevated,
        TriggerSignal::StakesFoundational => signals.stakes == Stakes::Foundational,
        TriggerSignal::ConfidenceBelow05 => signals.confidence.map(|c| c < 0.5).unwrap_or(false),
        TriggerSignal::ConfidenceBelow07 => signals.confidence.map(|c| c < 0.7).unwrap_or(false),
        TriggerSignal::ConfidenceBelow09 => signals.confidence.map(|c| c < 0.9).unwrap_or(false),
        TriggerSignal::AuditPass => signals.audit_pass == Some(true),
        TriggerSignal::AuditFail => signals.audit_pass == Some(false),
        TriggerSignal::PriorAttemptsGe1 => signals.prior_attempts >= 1,
        TriggerSignal::PriorAttemptsGe2 => signals.prior_attempts >= 2,
        TriggerSignal::PriorAttemptsGe3 => signals.prior_attempts >= 3,
        TriggerSignal::PriorAttemptsGe4 => signals.prior_attempts >= 4,
        TriggerSignal::PriorAttemptsGe5 => signals.prior_attempts >= 5,
        TriggerSignal::FeedbackContradiction => signals
            .feedback_classes
            .iter()
            .any(|c| *c == FeedbackClass::Contradiction),
        TriggerSignal::FeedbackUnimplementableCritical => {
            signals
                .feedback_classes
                .iter()
                .any(|c| *c == FeedbackClass::Unimplementable)
                && signals.feedback_critical
        }
        TriggerSignal::FeedbackGap => signals
            .feedback_classes
            .iter()
            .any(|c| *c == FeedbackClass::Gap),
        TriggerSignal::FeedbackScopeIssue => signals
            .feedback_classes
            .iter()
            .any(|c| *c == FeedbackClass::ScopeIssue),
    }
}

/// Choose a trigger signal that justifies escalation to `step` given
/// `signals`. Returns the first matching trigger in step order. Used
/// to record `dec:escalation_reason` on the escalated session.
#[must_use]
pub fn pick_reason(step: &EscalationStep, signals: &SignalSet) -> Option<TriggerSignal> {
    step.triggers
        .iter()
        .find(|t| evaluate_trigger(**t, signals))
        .copied()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::bundle::Stakes;
    use crate::core::feedback::FeedbackClass;
    use crate::core::ontology::capability::Endpoint;
    use crate::core::ontology::verdict::Verdict;

    fn signals(
        stakes: Stakes,
        confidence: Option<f32>,
        classes: Vec<FeedbackClass>,
        critical: bool,
        prior: u32,
    ) -> SignalSet {
        SignalSet {
            stakes,
            audit_pass: None,
            feedback_classes: classes,
            feedback_critical: critical,
            confidence,
            prior_attempts: prior,
            verdict: confidence.map(|_| Verdict::AmendmentRequired),
        }
    }

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
        }
    }

    #[test]
    fn confidence_below_thresholds_fire_correctly() {
        let s = signals(Stakes::Routine, Some(0.6), vec![], false, 1);
        assert!(evaluate_trigger(TriggerSignal::ConfidenceBelow07, &s));
        assert!(!evaluate_trigger(TriggerSignal::ConfidenceBelow05, &s));
        assert!(evaluate_trigger(TriggerSignal::ConfidenceBelow09, &s));
    }

    #[test]
    fn missing_confidence_never_fires_threshold() {
        let s = signals(Stakes::Routine, None, vec![], false, 1);
        for t in [
            TriggerSignal::ConfidenceBelow05,
            TriggerSignal::ConfidenceBelow07,
            TriggerSignal::ConfidenceBelow09,
        ] {
            assert!(!evaluate_trigger(t, &s));
        }
    }

    #[test]
    fn stakes_triggers_match_exact_value() {
        let s = signals(Stakes::Foundational, None, vec![], false, 1);
        assert!(evaluate_trigger(TriggerSignal::StakesFoundational, &s));
        assert!(!evaluate_trigger(TriggerSignal::StakesElevated, &s));
        assert!(!evaluate_trigger(TriggerSignal::StakesRoutine, &s));
    }

    #[test]
    fn feedback_unimplementable_critical_requires_both() {
        let s = signals(
            Stakes::Routine,
            None,
            vec![FeedbackClass::Unimplementable],
            true,
            1,
        );
        assert!(evaluate_trigger(
            TriggerSignal::FeedbackUnimplementableCritical,
            &s
        ));
        let s2 = signals(
            Stakes::Routine,
            None,
            vec![FeedbackClass::Unimplementable],
            false,
            1,
        );
        assert!(!evaluate_trigger(
            TriggerSignal::FeedbackUnimplementableCritical,
            &s2
        ));
    }

    #[test]
    fn self_escalation_skipped() {
        let cap_iri = NamedNode::new_unchecked(
            "https://decision-cli.dev/ns/capability/code-writer/v1",
        );
        let binding = RoleBinding {
            role_id: "verifier".to_string(),
            default_capability: cap_iri.clone(),
            escalation_steps: vec![EscalationStep {
                step_capability: cap_iri,
                triggers: vec![TriggerSignal::ConfidenceBelow07],
            }],
            version: 1,
            active: true,
            supersedes: None,
            bootstrap_source: None,
        };
        let s = signals(Stakes::Routine, Some(0.4), vec![], false, 1);
        assert!(find_next_escalation_step(&binding, &cap("code-writer", 1), &s).is_none());
    }

    #[test]
    fn pick_reason_returns_first_matching_trigger() {
        let step = EscalationStep {
            step_capability: NamedNode::new_unchecked(
                "https://decision-cli.dev/ns/capability/standard-reasoning-frontier/v1",
            ),
            triggers: vec![
                TriggerSignal::ConfidenceBelow05,
                TriggerSignal::ConfidenceBelow07,
            ],
        };
        let s = signals(Stakes::Routine, Some(0.6), vec![], false, 1);
        // 0.5 does not fire (confidence is 0.6); 0.7 does.
        assert_eq!(pick_reason(&step, &s), Some(TriggerSignal::ConfidenceBelow07));
    }
}
