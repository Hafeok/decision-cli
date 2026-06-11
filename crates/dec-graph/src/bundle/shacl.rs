//! Write-side SHACL validation for `dec:Bundle` (FT-056 / ADR-035).
//!
//! Mirrors the sibling validators (verdict, feedback, role-binding) in
//! shape: per-subject checks against the `dec:BundleShape` invariants —
//! exactly one `dec:stakes` literal drawn from the ADR-035 closed enum.

use oxigraph::model::{NamedNode, Quad, Subject, Term};
use thiserror::Error;

use dec_ontology::vocab::{
    IRI_DEC_BUNDLE, IRI_DEC_STAKES, STAKES_ELEVATED, STAKES_FOUNDATIONAL, STAKES_ROUTINE,
};

use super::types::RDF_TYPE;

/// One SHACL violation against a candidate bundle mutation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BundleViolation {
    /// Subject IRI the violation is attached to.
    pub subject: String,
    /// Predicate path the violation is against.
    pub path: String,
    /// Operator-friendly explanation.
    pub detail: String,
}

/// Structured failure for SHACL validation of a `Bundle`.
#[derive(Debug, Error)]
#[error("SHACL validation failed for Bundle:\n{report}")]
pub struct BundleShaclError {
    /// Rendered report (one `subject / path / detail` line per violation).
    pub report: String,
    /// The raw violations, in input order.
    pub violations: Vec<BundleViolation>,
}

/// Run the FT-056 SHACL shape against every `Bundle` subject declared in
/// `quads`. Returns `Ok(())` when no `Bundle` subjects appear (the shape
/// is targetted; un-typed subjects are out of scope).
pub fn validate_quads(quads: &[Quad]) -> Result<(), BundleShaclError> {
    let bundles = subjects_of_type(quads, IRI_DEC_BUNDLE);
    let mut violations: Vec<BundleViolation> = Vec::new();
    for subject in &bundles {
        violations.extend(check_bundle_subject(quads, subject));
    }
    if violations.is_empty() {
        return Ok(());
    }
    Err(BundleShaclError {
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

fn check_bundle_subject(quads: &[Quad], subject: &NamedNode) -> Vec<BundleViolation> {
    let stakes = literal_values(quads, subject, IRI_DEC_STAKES);
    let mut v = Vec::new();
    match stakes.len() {
        0 => v.push(violation(
            subject,
            IRI_DEC_STAKES,
            "missing required dec:stakes (sh:minCount 1)",
        )),
        1 => {
            let s = &stakes[0];
            if !is_in_vocabulary(s) {
                v.push(violation(
                    subject,
                    IRI_DEC_STAKES,
                    &format!(
                        "dec:stakes {s:?} is not in the ADR-035 closed enum (expected one of \
                         routine | elevated | foundational)"
                    ),
                ));
            }
        }
        n => v.push(violation(
            subject,
            IRI_DEC_STAKES,
            &format!("expected exactly one dec:stakes literal, found {n}"),
        )),
    }
    v
}

fn is_in_vocabulary(s: &str) -> bool {
    matches!(s, STAKES_ROUTINE | STAKES_ELEVATED | STAKES_FOUNDATIONAL)
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

fn violation(subject: &NamedNode, path: &str, detail: &str) -> BundleViolation {
    BundleViolation {
        subject: subject.as_str().to_string(),
        path: path.to_string(),
        detail: detail.to_string(),
    }
}

fn render_violations(violations: &[BundleViolation]) -> String {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bundle::{BundleBuilder, Stakes};
    use dec_ontology::vocab::{bundle_graph, stakes_pred, IRI_DEC_STAKES};
    use oxigraph::model::{GraphName, Literal, NamedNodeRef, Quad};

    fn focal() -> NamedNode {
        NamedNode::new("https://example.com/focal").expect("focal iri")
    }

    #[test]
    fn well_formed_bundle_passes_for_each_stakes_value() {
        for stakes in [Stakes::Routine, Stakes::Elevated, Stakes::Foundational] {
            let b = BundleBuilder::new("hash-1", focal())
                .with_stakes(stakes)
                .build();
            let quads = b.to_quads(bundle_graph());
            validate_quads(&quads).expect("well-formed bundle passes");
        }
    }

    #[test]
    fn bundle_with_unknown_stakes_value_fails() {
        let b = BundleBuilder::new("hash-2", focal()).build();
        let mut quads = b.to_quads(bundle_graph());
        // Mutate the stakes literal to "critical" — not in the enum.
        for q in quads.iter_mut() {
            if q.predicate.as_str() == IRI_DEC_STAKES {
                q.object = Literal::new_simple_literal("critical").into();
            }
        }
        let err = validate_quads(&quads).expect_err("must fail");
        assert!(err.report.contains("critical"), "{}", err.report);
        assert!(err.report.contains("stakes"), "{}", err.report);
    }

    #[test]
    fn bundle_with_missing_stakes_fails() {
        let b = BundleBuilder::new("hash-3", focal()).build();
        let quads = b.to_quads(bundle_graph());
        // Strip the stakes triple.
        let pruned: Vec<Quad> = quads
            .into_iter()
            .filter(|q| q.predicate.as_str() != IRI_DEC_STAKES)
            .collect();
        let err = validate_quads(&pruned).expect_err("must fail");
        assert!(err.report.contains("missing"), "{}", err.report);
        assert!(err.report.contains("stakes"), "{}", err.report);
    }

    #[test]
    fn bundle_with_two_stakes_literals_fails() {
        let b = BundleBuilder::new("hash-4", focal())
            .with_stakes(Stakes::Routine)
            .build();
        let mut quads = b.to_quads(bundle_graph());
        // Add a second (conflicting) stakes triple.
        let graph: GraphName = bundle_graph().into_owned().into();
        quads.push(Quad::new(
            b.iri(),
            stakes_pred().into_owned(),
            Literal::new_simple_literal(Stakes::Foundational.as_str()),
            graph,
        ));
        let err = validate_quads(&quads).expect_err("two stakes literals must fail");
        assert!(err.report.contains("exactly one"), "{}", err.report);
    }

    #[test]
    fn no_bundle_subjects_means_no_violations() {
        let _ = NamedNodeRef::new("https://example.com/x");
        validate_quads(&[]).expect("empty quads pass");
    }
}
