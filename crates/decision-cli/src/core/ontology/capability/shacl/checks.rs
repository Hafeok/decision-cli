//! Per-property SHACL checks for a single `dec:Capability` subject.

use std::collections::BTreeSet;

use oxigraph::model::{NamedNode, Quad};

use crate::core::vocab::{
    IRI_DEC_CAPABILITY_VERSION, IRI_DEC_COST_CACHE_HIT_PER_M, IRI_DEC_COST_CACHE_WRITE_5M,
    IRI_DEC_TIER,
};

use super::helpers::{literal_values, violation};
use super::CapabilityViolation;

pub(super) fn check_string_one(
    quads: &[Quad],
    subject: &NamedNode,
    predicate: &str,
    label: &str,
    required: bool,
    violations: &mut Vec<CapabilityViolation>,
) {
    let values = literal_values(quads, subject, predicate);
    if values.is_empty() {
        if required {
            violations.push(violation(
                subject,
                predicate,
                &format!("missing required {label} (sh:minCount 1)"),
            ));
        }
        return;
    }
    if values.len() > 1 {
        violations.push(violation(
            subject,
            predicate,
            &format!("expected exactly one {label}, found {}", values.len()),
        ));
    }
    if required && values.iter().any(String::is_empty) {
        violations.push(violation(
            subject,
            predicate,
            &format!("{label} must be a non-empty string"),
        ));
    }
}

pub(super) fn check_optional_string(
    quads: &[Quad],
    subject: &NamedNode,
    predicate: &str,
    label: &str,
    violations: &mut Vec<CapabilityViolation>,
) {
    let values = literal_values(quads, subject, predicate);
    if values.len() > 1 {
        violations.push(violation(
            subject,
            predicate,
            &format!("expected at most one {label}, found {}", values.len()),
        ));
    }
}

pub(super) fn check_optional_iri(
    quads: &[Quad],
    subject: &NamedNode,
    predicate: &str,
    label: &str,
    violations: &mut Vec<CapabilityViolation>,
) {
    let values = super::helpers::iri_values(quads, subject, predicate);
    if values.len() > 1 {
        violations.push(violation(
            subject,
            predicate,
            &format!("expected at most one {label}, found {}", values.len()),
        ));
    }
}

pub(super) fn check_enum_one(
    quads: &[Quad],
    subject: &NamedNode,
    predicate: &str,
    label: &str,
    allowed: &[&str],
    violations: &mut Vec<CapabilityViolation>,
) {
    let values = literal_values(quads, subject, predicate);
    if values.is_empty() {
        violations.push(violation(
            subject,
            predicate,
            &format!("missing required {label} (sh:minCount 1)"),
        ));
        return;
    }
    if values.len() > 1 {
        violations.push(violation(
            subject,
            predicate,
            &format!("expected exactly one {label}, found {}", values.len()),
        ));
    }
    let allowed_set: BTreeSet<&str> = allowed.iter().copied().collect();
    for v in &values {
        if !allowed_set.contains(v.as_str()) {
            violations.push(violation(
                subject,
                predicate,
                &format!(
                    "{label} must be one of {{{joined}}}, got {v:?}",
                    joined = allowed.join(", "),
                ),
            ));
        }
    }
}

pub(super) fn check_non_negative_int_one(
    quads: &[Quad],
    subject: &NamedNode,
    predicate: &str,
    label: &str,
    required: bool,
    violations: &mut Vec<CapabilityViolation>,
) {
    let values = literal_values(quads, subject, predicate);
    if values.is_empty() {
        if required {
            violations.push(violation(
                subject,
                predicate,
                &format!("missing required {label} (sh:minCount 1)"),
            ));
        }
        return;
    }
    if values.len() > 1 {
        violations.push(violation(
            subject,
            predicate,
            &format!("expected exactly one {label}, found {}", values.len()),
        ));
    }
    for v in &values {
        match v.parse::<i64>() {
            Ok(n) if n >= 0 => {}
            Ok(n) => violations.push(violation(
                subject,
                predicate,
                &format!("{label} must be ≥ 0, got {n}"),
            )),
            Err(_) => violations.push(violation(
                subject,
                predicate,
                &format!("{label} must be an integer, got {v:?}"),
            )),
        }
    }
}

pub(super) fn check_non_negative_decimal_one(
    quads: &[Quad],
    subject: &NamedNode,
    predicate: &str,
    label: &str,
    required: bool,
    violations: &mut Vec<CapabilityViolation>,
) {
    let values = literal_values(quads, subject, predicate);
    if values.is_empty() {
        if required {
            violations.push(violation(
                subject,
                predicate,
                &format!("missing required {label} (sh:minCount 1)"),
            ));
        }
        return;
    }
    if values.len() > 1 {
        violations.push(violation(
            subject,
            predicate,
            &format!("expected exactly one {label}, found {}", values.len()),
        ));
    }
    for v in &values {
        match v.parse::<f64>() {
            Ok(n) if n >= 0.0 => {}
            Ok(n) => violations.push(violation(
                subject,
                predicate,
                &format!("{label} must be ≥ 0, got {n}"),
            )),
            Err(_) => violations.push(violation(
                subject,
                predicate,
                &format!("{label} must be a decimal, got {v:?}"),
            )),
        }
    }
}

pub(super) fn check_boolean_one(
    quads: &[Quad],
    subject: &NamedNode,
    predicate: &str,
    label: &str,
    required: bool,
    violations: &mut Vec<CapabilityViolation>,
) {
    let values = literal_values(quads, subject, predicate);
    if values.is_empty() {
        if required {
            violations.push(violation(
                subject,
                predicate,
                &format!("missing required {label} (sh:minCount 1)"),
            ));
        }
        return;
    }
    if values.len() > 1 {
        violations.push(violation(
            subject,
            predicate,
            &format!("expected exactly one {label}, found {}", values.len()),
        ));
    }
    for v in &values {
        if !matches!(v.as_str(), "true" | "false" | "0" | "1") {
            violations.push(violation(
                subject,
                predicate,
                &format!("{label} must be xsd:boolean, got {v:?}"),
            ));
        }
    }
}

pub(super) fn check_version_ge_one(
    quads: &[Quad],
    subject: &NamedNode,
    violations: &mut Vec<CapabilityViolation>,
) {
    let values = literal_values(quads, subject, IRI_DEC_CAPABILITY_VERSION);
    if values.is_empty() {
        violations.push(violation(
            subject,
            IRI_DEC_CAPABILITY_VERSION,
            "missing required dec:version (sh:minCount 1)",
        ));
        return;
    }
    if values.len() > 1 {
        violations.push(violation(
            subject,
            IRI_DEC_CAPABILITY_VERSION,
            &format!("expected exactly one dec:version, found {}", values.len()),
        ));
    }
    for v in &values {
        match v.parse::<i64>() {
            Ok(n) if n >= 1 => {}
            Ok(n) => violations.push(violation(
                subject,
                IRI_DEC_CAPABILITY_VERSION,
                &format!("dec:version must be ≥ 1, got {n}"),
            )),
            Err(_) => violations.push(violation(
                subject,
                IRI_DEC_CAPABILITY_VERSION,
                &format!("dec:version must be an integer, got {v:?}"),
            )),
        }
    }
}

pub(super) fn check_tier_range(
    quads: &[Quad],
    subject: &NamedNode,
    violations: &mut Vec<CapabilityViolation>,
) {
    let values = literal_values(quads, subject, IRI_DEC_TIER);
    if values.len() > 1 {
        violations.push(violation(
            subject,
            IRI_DEC_TIER,
            &format!("expected at most one dec:tier, found {}", values.len()),
        ));
    }
    for v in &values {
        match v.parse::<i64>() {
            Ok(n) if (0..=3).contains(&n) => {}
            Ok(n) => violations.push(violation(
                subject,
                IRI_DEC_TIER,
                &format!("dec:tier must be in 0..=3, got {n}"),
            )),
            Err(_) => violations.push(violation(
                subject,
                IRI_DEC_TIER,
                &format!("dec:tier must be an integer, got {v:?}"),
            )),
        }
    }
}

pub(super) fn check_cache_cost_pair(
    quads: &[Quad],
    subject: &NamedNode,
    violations: &mut Vec<CapabilityViolation>,
) {
    let has_hit = !literal_values(quads, subject, IRI_DEC_COST_CACHE_HIT_PER_M).is_empty();
    let has_write = !literal_values(quads, subject, IRI_DEC_COST_CACHE_WRITE_5M).is_empty();
    if has_hit != has_write {
        violations.push(violation(
            subject,
            IRI_DEC_COST_CACHE_HIT_PER_M,
            "dec:cost_cache_hit_per_m and dec:cost_cache_write_5m must be paired (both present or both absent)",
        ));
    }
}
