//! Per-subject FT-057 SHACL check functions.

use std::collections::HashMap;

use oxigraph::model::{NamedNode, Quad};

use crate::ontology::role_binding::TriggerSignal;
use dec_ontology::vocab::{
    ENDPOINT_SCALEWAY, IRI_DEC_ESCALATED_FROM, IRI_DEC_ESCALATED_TO, IRI_DEC_ESCALATION_REASON,
    IRI_DEC_INPUT_TOKENS_BASE, IRI_DEC_INPUT_TOKENS_CACHE_HIT, IRI_DEC_INPUT_TOKENS_CACHE_WRITE,
    IRI_DEC_OUTPUT_TOKENS, IRI_DEC_SESSION_CAPABILITY,
};

use super::helpers::{iri_objects_for, literal_objects_for, violation, SessionRecordViolation};

pub(super) fn check_escalation_reason_iff_from(
    quads: &[Quad],
    subject: &NamedNode,
    violations: &mut Vec<SessionRecordViolation>,
) {
    let has_from = !iri_objects_for(quads, subject, IRI_DEC_ESCALATED_FROM).is_empty();
    let has_reason = !literal_objects_for(quads, subject, IRI_DEC_ESCALATION_REASON).is_empty();
    if has_from && !has_reason {
        violations.push(violation(
            subject,
            IRI_DEC_ESCALATION_REASON,
            "dec:escalated_from is set but dec:escalation_reason is absent (FT-057 §SHACL)",
        ));
    }
    if has_reason && !has_from {
        violations.push(violation(
            subject,
            IRI_DEC_ESCALATION_REASON,
            "dec:escalation_reason is set but dec:escalated_from is absent (FT-057 §SHACL)",
        ));
    }
}

pub(super) fn check_bidirectional_escalation(
    quads: &[Quad],
    subject: &NamedNode,
    violations: &mut Vec<SessionRecordViolation>,
) {
    for prior_iri in iri_objects_for(quads, subject, IRI_DEC_ESCALATED_FROM) {
        let prior = NamedNode::new_unchecked(&prior_iri);
        let mirrored = iri_objects_for(quads, &prior, IRI_DEC_ESCALATED_TO)
            .into_iter()
            .any(|iri| iri == subject.as_str());
        if !mirrored {
            violations.push(violation(
                subject,
                IRI_DEC_ESCALATED_TO,
                &format!(
                    "dec:escalated_from {prior_iri:?} is set but the prior session is missing the inverse dec:escalated_to {iri:?} triple (FT-057 §SHACL bidirectional)",
                    iri = subject.as_str(),
                ),
            ));
        }
    }
    for next_iri in iri_objects_for(quads, subject, IRI_DEC_ESCALATED_TO) {
        let next = NamedNode::new_unchecked(&next_iri);
        let mirrored = iri_objects_for(quads, &next, IRI_DEC_ESCALATED_FROM)
            .into_iter()
            .any(|iri| iri == subject.as_str());
        if !mirrored {
            violations.push(violation(
                subject,
                IRI_DEC_ESCALATED_FROM,
                &format!(
                    "dec:escalated_to {next_iri:?} is set but the successor session is missing the inverse dec:escalated_from {iri:?} triple (FT-057 §SHACL bidirectional)",
                    iri = subject.as_str(),
                ),
            ));
        }
    }
}

pub(super) fn check_escalation_reason_vocabulary(
    quads: &[Quad],
    subject: &NamedNode,
    violations: &mut Vec<SessionRecordViolation>,
) {
    let reasons = literal_objects_for(quads, subject, IRI_DEC_ESCALATION_REASON);
    if reasons.len() > 1 {
        violations.push(violation(
            subject,
            IRI_DEC_ESCALATION_REASON,
            &format!(
                "dec:escalation_reason must occur at most once, found {n}",
                n = reasons.len(),
            ),
        ));
    }
    for reason in reasons {
        if TriggerSignal::try_from_str(&reason).is_none() {
            violations.push(violation(
                subject,
                IRI_DEC_ESCALATION_REASON,
                &format!(
                    "dec:escalation_reason {reason:?} is not in the ADR-034 trigger vocabulary"
                ),
            ));
        }
    }
}

pub(super) fn check_token_fields_non_negative(
    quads: &[Quad],
    subject: &NamedNode,
    violations: &mut Vec<SessionRecordViolation>,
) {
    for pred in &[
        IRI_DEC_INPUT_TOKENS_BASE,
        IRI_DEC_INPUT_TOKENS_CACHE_WRITE,
        IRI_DEC_INPUT_TOKENS_CACHE_HIT,
        IRI_DEC_OUTPUT_TOKENS,
    ] {
        let values = literal_objects_for(quads, subject, pred);
        if values.len() > 1 {
            violations.push(violation(
                subject,
                pred,
                &format!(
                    "{pred} must occur at most once, found {n}",
                    n = values.len(),
                ),
            ));
        }
        for value in values {
            match value.parse::<i64>() {
                Ok(n) if n >= 0 => {}
                Ok(n) => violations.push(violation(
                    subject,
                    pred,
                    &format!("{pred} must be a non-negative integer, got {n}"),
                )),
                Err(_) => violations.push(violation(
                    subject,
                    pred,
                    &format!("{pred} must be a non-negative integer, got {value:?}"),
                )),
            }
        }
    }
}

pub(super) fn check_scaleway_no_cache(
    quads: &[Quad],
    subject: &NamedNode,
    endpoints: &HashMap<String, String>,
    violations: &mut Vec<SessionRecordViolation>,
) {
    let Some(cap_iri) = iri_objects_for(quads, subject, IRI_DEC_SESSION_CAPABILITY)
        .into_iter()
        .next()
    else {
        return;
    };
    let Some(endpoint) = endpoints.get(&cap_iri) else {
        return;
    };
    if endpoint != ENDPOINT_SCALEWAY {
        return;
    }
    for (pred, label) in &[
        (
            IRI_DEC_INPUT_TOKENS_CACHE_WRITE,
            "dec:input_tokens_cache_write",
        ),
        (IRI_DEC_INPUT_TOKENS_CACHE_HIT, "dec:input_tokens_cache_hit"),
    ] {
        for value in literal_objects_for(quads, subject, pred) {
            let parsed = value.parse::<i64>().unwrap_or(-1);
            if parsed != 0 {
                violations.push(violation(
                    subject,
                    pred,
                    &format!(
                        "{label} must be 0 for scaleway dispatches (capability {cap_iri} endpoint=scaleway), got {value:?} (FT-057 / ADR-037)"
                    ),
                ));
            }
        }
    }
}
