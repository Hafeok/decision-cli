//! Write-side SHACL validation for `dec:RoleBinding`, `dec:EscalationStep`,
//! `dec:EscalationTrigger` (FT-055 / ADR-033 / ADR-034).
//!
//! Mirrors the capability validator structure: per-subject checks plus a
//! cross-subject pass for the at-most-one-active-per-role invariant. The
//! `default_capability.status ≠ eol` check is enforced when the relevant
//! capability quads are in scope (SHACL semantics fire when there is data
//! to evaluate; the StreamWriter passes the full mutation insert set).

use std::collections::BTreeMap;

use oxigraph::model::{NamedNode, Quad, Subject, Term};
use thiserror::Error;

use crate::core::vocab::{
    CAPABILITY_STATUS_EOL, IRI_DEC_CAPABILITY_STATUS, IRI_DEC_CAPABILITY_VERSION,
    IRI_DEC_DEFAULT_CAPABILITY, IRI_DEC_ESCALATION_STEP, IRI_DEC_ESCALATION_TRIGGER,
    IRI_DEC_ROLE_BINDING, IRI_DEC_ROLE_BINDING_ACTIVE, IRI_DEC_ROLE_BINDING_ROLE_ID,
    IRI_DEC_STEP_CAPABILITY, IRI_DEC_TRIGGERS, IRI_DEC_TRIGGER_SIGNAL,
    TRIGGER_SIGNAL_VOCABULARY,
};

use super::types::RDF_TYPE;

/// One SHACL violation against a candidate role-binding mutation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoleBindingViolation {
    /// Subject IRI the violation is attached to.
    pub subject: String,
    /// Predicate path the violation is against.
    pub path: String,
    /// Operator-friendly explanation.
    pub detail: String,
}

/// Structured failure for SHACL validation of a `RoleBinding`.
#[derive(Debug, Error)]
#[error("SHACL validation failed for RoleBinding:\n{report}")]
pub struct RoleBindingShaclError {
    /// Rendered report (one `subject / path / detail` line per violation).
    pub report: String,
    /// The raw violations, in input order.
    pub violations: Vec<RoleBindingViolation>,
}

/// Run the FT-055 SHACL shapes against every `RoleBinding` /
/// `EscalationStep` / `EscalationTrigger` subject declared in `quads`.
pub fn validate_quads(quads: &[Quad]) -> Result<(), RoleBindingShaclError> {
    let mut violations: Vec<RoleBindingViolation> = Vec::new();
    let bindings = subjects_of_type(quads, IRI_DEC_ROLE_BINDING);
    let steps = subjects_of_type(quads, IRI_DEC_ESCALATION_STEP);
    let triggers = subjects_of_type(quads, IRI_DEC_ESCALATION_TRIGGER);

    for s in &bindings {
        violations.extend(check_binding_subject(quads, s));
    }
    for s in &steps {
        violations.extend(check_step_subject(quads, s));
    }
    for s in &triggers {
        violations.extend(check_trigger_subject(quads, s));
    }
    violations.extend(check_active_unique(quads, &bindings));
    violations.extend(check_default_capability_not_eol(quads, &bindings));

    if violations.is_empty() {
        return Ok(());
    }
    Err(RoleBindingShaclError {
        report: render_violations(&violations),
        violations,
    })
}

fn subjects_of_type(quads: &[Quad], class_iri: &str) -> Vec<NamedNode> {
    let mut out: Vec<NamedNode> = Vec::new();
    for q in quads {
        if q.predicate.as_str() != RDF_TYPE {
            continue;
        }
        let Term::NamedNode(cls) = &q.object else {
            continue;
        };
        if cls.as_str() != class_iri {
            continue;
        }
        let Subject::NamedNode(s) = &q.subject else {
            continue;
        };
        if !out.iter().any(|n| n == s) {
            out.push(s.clone());
        }
    }
    out
}

fn check_binding_subject(quads: &[Quad], subject: &NamedNode) -> Vec<RoleBindingViolation> {
    let mut v = Vec::new();
    // dec:role_id — exactly one non-empty string.
    let ids = literal_values(quads, subject, IRI_DEC_ROLE_BINDING_ROLE_ID);
    match ids.len() {
        0 => v.push(violation(
            subject,
            IRI_DEC_ROLE_BINDING_ROLE_ID,
            "missing required dec:role_id (sh:minCount 1)",
        )),
        1 => {
            if ids[0].is_empty() {
                v.push(violation(
                    subject,
                    IRI_DEC_ROLE_BINDING_ROLE_ID,
                    "dec:role_id must be a non-empty string",
                ));
            }
        }
        n => v.push(violation(
            subject,
            IRI_DEC_ROLE_BINDING_ROLE_ID,
            &format!("expected exactly one dec:role_id, found {n}"),
        )),
    }
    // dec:default_capability — exactly one IRI.
    let caps = iri_values(quads, subject, IRI_DEC_DEFAULT_CAPABILITY);
    match caps.len() {
        0 => v.push(violation(
            subject,
            IRI_DEC_DEFAULT_CAPABILITY,
            "missing required dec:default_capability (sh:minCount 1)",
        )),
        1 => {}
        n => v.push(violation(
            subject,
            IRI_DEC_DEFAULT_CAPABILITY,
            &format!("expected exactly one dec:default_capability, found {n}"),
        )),
    }
    // dec:active — exactly one boolean.
    let actives = literal_values(quads, subject, IRI_DEC_ROLE_BINDING_ACTIVE);
    match actives.len() {
        0 => v.push(violation(
            subject,
            IRI_DEC_ROLE_BINDING_ACTIVE,
            "missing required dec:active (sh:minCount 1)",
        )),
        1 => {
            if !matches!(actives[0].as_str(), "true" | "false" | "0" | "1") {
                v.push(violation(
                    subject,
                    IRI_DEC_ROLE_BINDING_ACTIVE,
                    &format!("dec:active must be xsd:boolean, got {:?}", actives[0]),
                ));
            }
        }
        n => v.push(violation(
            subject,
            IRI_DEC_ROLE_BINDING_ACTIVE,
            &format!("expected exactly one dec:active, found {n}"),
        )),
    }
    // dec:version — exactly one integer ≥ 1.
    let versions = literal_values(quads, subject, IRI_DEC_CAPABILITY_VERSION);
    match versions.len() {
        0 => v.push(violation(
            subject,
            IRI_DEC_CAPABILITY_VERSION,
            "missing required dec:version (sh:minCount 1)",
        )),
        1 => match versions[0].parse::<i64>() {
            Ok(n) if n >= 1 => {}
            Ok(n) => v.push(violation(
                subject,
                IRI_DEC_CAPABILITY_VERSION,
                &format!("dec:version must be ≥ 1, got {n}"),
            )),
            Err(_) => v.push(violation(
                subject,
                IRI_DEC_CAPABILITY_VERSION,
                &format!("dec:version must be an integer, got {:?}", versions[0]),
            )),
        },
        n => v.push(violation(
            subject,
            IRI_DEC_CAPABILITY_VERSION,
            &format!("expected exactly one dec:version, found {n}"),
        )),
    }
    v
}

fn check_step_subject(quads: &[Quad], subject: &NamedNode) -> Vec<RoleBindingViolation> {
    let mut v = Vec::new();
    let caps = iri_values(quads, subject, IRI_DEC_STEP_CAPABILITY);
    match caps.len() {
        0 => v.push(violation(
            subject,
            IRI_DEC_STEP_CAPABILITY,
            "missing required dec:step_capability (sh:minCount 1)",
        )),
        1 => {}
        n => v.push(violation(
            subject,
            IRI_DEC_STEP_CAPABILITY,
            &format!("expected exactly one dec:step_capability, found {n}"),
        )),
    }
    let triggers = iri_values(quads, subject, IRI_DEC_TRIGGERS);
    if triggers.is_empty() {
        v.push(violation(
            subject,
            IRI_DEC_TRIGGERS,
            "dec:EscalationStep requires at least one dec:triggers (sh:minCount 1)",
        ));
    }
    v
}

fn check_trigger_subject(quads: &[Quad], subject: &NamedNode) -> Vec<RoleBindingViolation> {
    let mut v = Vec::new();
    let signals = literal_values(quads, subject, IRI_DEC_TRIGGER_SIGNAL);
    match signals.len() {
        0 => v.push(violation(
            subject,
            IRI_DEC_TRIGGER_SIGNAL,
            "missing required dec:trigger_signal (sh:minCount 1)",
        )),
        1 => {
            if !TRIGGER_SIGNAL_VOCABULARY.iter().any(|w| *w == signals[0]) {
                v.push(violation(
                    subject,
                    IRI_DEC_TRIGGER_SIGNAL,
                    &format!(
                        "dec:trigger_signal {:?} is not in the ADR-034 vocabulary",
                        signals[0]
                    ),
                ));
            }
        }
        n => v.push(violation(
            subject,
            IRI_DEC_TRIGGER_SIGNAL,
            &format!("expected exactly one dec:trigger_signal, found {n}"),
        )),
    }
    v
}

fn check_active_unique(quads: &[Quad], bindings: &[NamedNode]) -> Vec<RoleBindingViolation> {
    let mut by_role: BTreeMap<String, Vec<NamedNode>> = BTreeMap::new();
    for s in bindings {
        let actives = literal_values(quads, s, IRI_DEC_ROLE_BINDING_ACTIVE);
        if !actives.iter().any(|a| matches!(a.as_str(), "true" | "1")) {
            continue;
        }
        let ids = literal_values(quads, s, IRI_DEC_ROLE_BINDING_ROLE_ID);
        let Some(id) = ids.into_iter().next() else {
            continue;
        };
        by_role.entry(id).or_default().push(s.clone());
    }
    let mut out = Vec::new();
    for (role_id, nodes) in by_role {
        if nodes.len() <= 1 {
            continue;
        }
        for n in &nodes {
            out.push(violation(
                n,
                IRI_DEC_ROLE_BINDING_ACTIVE,
                &format!(
                    "more than one dec:RoleBinding marked dec:active=true for role_id={role_id:?} ({n_total} total)",
                    n_total = nodes.len(),
                ),
            ));
        }
    }
    out
}

fn check_default_capability_not_eol(
    quads: &[Quad],
    bindings: &[NamedNode],
) -> Vec<RoleBindingViolation> {
    // For each binding, look up its default_capability IRI and check
    // whether any quad in the same set declares dec:status "eol" on it.
    let mut out = Vec::new();
    for b in bindings {
        let caps = iri_values(quads, b, IRI_DEC_DEFAULT_CAPABILITY);
        for cap in caps {
            let cap_node = NamedNode::new_unchecked(cap.clone());
            let statuses = literal_values(quads, &cap_node, IRI_DEC_CAPABILITY_STATUS);
            if statuses.iter().any(|s| s == CAPABILITY_STATUS_EOL) {
                out.push(violation(
                    b,
                    IRI_DEC_DEFAULT_CAPABILITY,
                    &format!(
                        "dec:default_capability <{cap}> has dec:status=\"eol\"; bindings must reference a non-EOL capability"
                    ),
                ));
            }
        }
    }
    out
}

fn literal_values(quads: &[Quad], subject: &NamedNode, predicate: &str) -> Vec<String> {
    quads
        .iter()
        .filter_map(|q| {
            if q.predicate.as_str() != predicate {
                return None;
            }
            let Subject::NamedNode(s) = &q.subject else {
                return None;
            };
            if s != subject {
                return None;
            }
            match &q.object {
                Term::Literal(lit) => Some(lit.value().to_string()),
                _ => None,
            }
        })
        .collect()
}

fn iri_values(quads: &[Quad], subject: &NamedNode, predicate: &str) -> Vec<String> {
    quads
        .iter()
        .filter_map(|q| {
            if q.predicate.as_str() != predicate {
                return None;
            }
            let Subject::NamedNode(s) = &q.subject else {
                return None;
            };
            if s != subject {
                return None;
            }
            match &q.object {
                Term::NamedNode(n) => Some(n.as_str().to_string()),
                _ => None,
            }
        })
        .collect()
}

fn violation(subject: &NamedNode, path: &str, detail: &str) -> RoleBindingViolation {
    RoleBindingViolation {
        subject: subject.as_str().to_string(),
        path: path.to_string(),
        detail: detail.to_string(),
    }
}

fn render_violations(violations: &[RoleBindingViolation]) -> String {
    use std::fmt::Write;
    let mut out = String::new();
    for v in violations {
        let _ = writeln!(
            out,
            "  • subject <{}> path <{}>: {}",
            v.subject, v.path, v.detail
        );
    }
    out
}
