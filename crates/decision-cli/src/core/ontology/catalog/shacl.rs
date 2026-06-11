//! Write-side SHACL validation for the three FT-101 catalog types.
//!
//! Validates the mutation against the live orchestration store so the
//! cross-artifact invariants the ADR-066 substrate relies on are
//! structural rather than aspirational:
//!
//! - **CapabilityReference**: `dec:command` uniqueness across the
//!   non-superseded (active) set; supersession is the only path to
//!   evolve a command's reference (TC-165).
//! - **OntologyDescription**: at most one non-superseded
//!   `dec:OntologyDescription` per stream (TC-166).
//! - **ExemplarGraph**: every `dec:exemplarOf <VG>` must resolve to an
//!   approved `dec:VerificationGraphResult dec:resultOf <VG>` already
//!   committed to the store (TC-167). Rationale `sh:minLength 40`.
//! - **Supersession cycles** are detected (A→B and B→A in the union of
//!   mutation + store).
//!
//! Like the verification-result validator (FT-097), this validator runs
//! through the [`crate::StreamWriter`] chokepoint and reads the store
//! when one is supplied so it can see prior catalog artifacts that the
//! current mutation does not re-declare.

use std::collections::{BTreeMap, BTreeSet};

use oxigraph::model::{NamedNode, Quad, Subject, Term};
use oxigraph::sparql::QueryResults;
use oxigraph::store::Store;
use thiserror::Error;

use crate::core::vocab::{
    IRI_DEC_APPLIES_TO_SAFETY_CLASS, IRI_DEC_BASED_ON_APPROVED_RESULT, IRI_DEC_CAPABILITY_BODY,
    IRI_DEC_CAPABILITY_REFERENCE, IRI_DEC_CAPABILITY_VERSION_CR, IRI_DEC_COMMAND_CR,
    IRI_DEC_EXEMPLAR_GRAPH, IRI_DEC_EXEMPLAR_OF, IRI_DEC_GRAPH_CATALOG, IRI_DEC_NAMESPACE,
    IRI_DEC_ONTOLOGY_BODY, IRI_DEC_ONTOLOGY_DESCRIPTION, IRI_DEC_ONTOLOGY_VERSION,
    IRI_DEC_PATTERN_NAME, IRI_DEC_PREFIX, IRI_DEC_RATIONALE, SAFETY_ISOLATED,
    SAFETY_PRODUCTION_READONLY, SAFETY_SHARED_NON_DESTRUCTIVE, VERDICT_APPROVED,
};

use super::types::{RDF_TYPE, SUPERSEDED_BY, SUPERSEDES};

/// Minimum rationale length for an ExemplarGraph (ADR-066 / FT-101: the
/// rationale teaches the LLM what makes the pattern good — short
/// rationales are a code smell).
pub const EXEMPLAR_RATIONALE_MIN_LEN: usize = 40;

/// One violation against a candidate catalog mutation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogViolation {
    /// Short shape name (e.g. `CapabilityReferenceShape`).
    pub shape: String,
    /// Subject IRI the violation is attached to.
    pub subject: String,
    /// Predicate path (or pseudo-path like `dec:supersedes/cycle`).
    pub path: String,
    /// Operator-friendly explanation.
    pub detail: String,
}

/// Structured failure for SHACL validation of a catalog mutation.
#[derive(Debug, Error)]
#[error("SHACL validation failed for catalog mutation:\n{report}")]
pub struct CatalogShaclError {
    /// Rendered report (one `subject / path / detail` line per violation).
    pub report: String,
    /// The raw violations, in input order.
    pub violations: Vec<CatalogViolation>,
}

/// Convenience entry that runs the validator without a store snapshot.
/// Cross-store invariants (active-set uniqueness, exemplar proof) are
/// best-effort against the mutation alone.
pub fn validate_quads(quads: &[Quad]) -> Result<(), CatalogShaclError> {
    validate_quads_with_store(quads, None)
}

/// Validate `quads` against the catalog SHACL contract, consulting
/// `store` for cross-mutation invariants when supplied.
pub fn validate_quads_with_store(
    quads: &[Quad],
    store: Option<&Store>,
) -> Result<(), CatalogShaclError> {
    let mut violations: Vec<CatalogViolation> = Vec::new();

    let cr_subjects = subjects_of_type(quads, IRI_DEC_CAPABILITY_REFERENCE);
    for s in &cr_subjects {
        violations.extend(validate_capability_reference(quads, s));
    }
    violations.extend(check_capability_active_command_unique(
        quads,
        &cr_subjects,
        store,
    ));

    let od_subjects = subjects_of_type(quads, IRI_DEC_ONTOLOGY_DESCRIPTION);
    for s in &od_subjects {
        violations.extend(validate_ontology_description(quads, s));
    }
    violations.extend(check_ontology_single_active(quads, &od_subjects, store));

    let ex_subjects = subjects_of_type(quads, IRI_DEC_EXEMPLAR_GRAPH);
    for s in &ex_subjects {
        violations.extend(validate_exemplar_graph(quads, s));
        violations.extend(check_exemplar_backed_by_approved(quads, s, store));
    }

    violations.extend(check_supersession_cycles(quads, store));

    if violations.is_empty() {
        Ok(())
    } else {
        Err(CatalogShaclError {
            report: render(&violations),
            violations,
        })
    }
}

// ---------------------------------------------------------------------------
// Per-type field validation
// ---------------------------------------------------------------------------

fn validate_capability_reference(quads: &[Quad], subject: &NamedNode) -> Vec<CatalogViolation> {
    let mut out = Vec::new();
    require_single_literal(
        quads,
        subject,
        IRI_DEC_COMMAND_CR,
        "CapabilityReferenceShape",
        "dec:command",
        &mut out,
    );
    require_single_literal(
        quads,
        subject,
        IRI_DEC_CAPABILITY_VERSION_CR,
        "CapabilityReferenceShape",
        "dec:capabilityVersion",
        &mut out,
    );
    require_single_literal(
        quads,
        subject,
        IRI_DEC_CAPABILITY_BODY,
        "CapabilityReferenceShape",
        "dec:capabilityBody",
        &mut out,
    );
    out
}

fn validate_ontology_description(quads: &[Quad], subject: &NamedNode) -> Vec<CatalogViolation> {
    let mut out = Vec::new();
    require_single_literal(
        quads,
        subject,
        IRI_DEC_NAMESPACE,
        "OntologyDescriptionShape",
        "dec:namespace",
        &mut out,
    );
    require_single_literal(
        quads,
        subject,
        IRI_DEC_PREFIX,
        "OntologyDescriptionShape",
        "dec:prefix",
        &mut out,
    );
    require_single_literal(
        quads,
        subject,
        IRI_DEC_ONTOLOGY_VERSION,
        "OntologyDescriptionShape",
        "dec:ontologyVersion",
        &mut out,
    );
    require_single_literal(
        quads,
        subject,
        IRI_DEC_ONTOLOGY_BODY,
        "OntologyDescriptionShape",
        "dec:ontologyBody",
        &mut out,
    );
    out
}

fn validate_exemplar_graph(quads: &[Quad], subject: &NamedNode) -> Vec<CatalogViolation> {
    let mut out = Vec::new();
    if !has_any_predicate(quads, subject, IRI_DEC_EXEMPLAR_OF) {
        out.push(violation(
            "ExemplarGraphShape",
            subject,
            "dec:exemplarOf",
            "missing required dec:exemplarOf link to a dec:VerificationGraph (sh:minCount 1)",
        ));
    }
    if !has_any_predicate(quads, subject, IRI_DEC_BASED_ON_APPROVED_RESULT) {
        out.push(violation(
            "ExemplarGraphShape",
            subject,
            "dec:basedOnApprovedResult",
            "missing required dec:basedOnApprovedResult — exemplar promotion requires a backing approved VerificationGraphResult (ADR-066)",
        ));
    }
    require_single_literal(
        quads,
        subject,
        IRI_DEC_PATTERN_NAME,
        "ExemplarGraphShape",
        "dec:patternName",
        &mut out,
    );
    let safety_vals = literal_values(quads, subject, IRI_DEC_APPLIES_TO_SAFETY_CLASS);
    if safety_vals.is_empty() {
        out.push(violation(
            "ExemplarGraphShape",
            subject,
            "dec:appliesToSafetyClass",
            "missing required dec:appliesToSafetyClass (sh:minCount 1)",
        ));
    } else {
        let allowed: BTreeSet<&str> = [
            SAFETY_ISOLATED,
            SAFETY_SHARED_NON_DESTRUCTIVE,
            SAFETY_PRODUCTION_READONLY,
        ]
        .into_iter()
        .collect();
        for v in &safety_vals {
            if !allowed.contains(v.as_str()) {
                out.push(violation(
                    "ExemplarGraphShape",
                    subject,
                    "dec:appliesToSafetyClass",
                    &format!(
                        "dec:appliesToSafetyClass must be one of {{isolated, shared-non-destructive, production-readonly}}, got {v:?}"
                    ),
                ));
            }
        }
    }
    let rat_vals = literal_values(quads, subject, IRI_DEC_RATIONALE);
    if rat_vals.is_empty() {
        out.push(violation(
            "ExemplarGraphShape",
            subject,
            "dec:rationale",
            "missing required dec:rationale (sh:minCount 1)",
        ));
    } else {
        for v in &rat_vals {
            if v.chars().count() < EXEMPLAR_RATIONALE_MIN_LEN {
                out.push(violation(
                    "ExemplarGraphShape",
                    subject,
                    "dec:rationale",
                    &format!(
                        "dec:rationale must be at least {EXEMPLAR_RATIONALE_MIN_LEN} characters (sh:minLength {EXEMPLAR_RATIONALE_MIN_LEN}); got {n}",
                        n = v.chars().count()
                    ),
                ));
            }
        }
    }
    out
}

// ---------------------------------------------------------------------------
// CapabilityReference: active-set command uniqueness (TC-165)
// ---------------------------------------------------------------------------

fn check_capability_active_command_unique(
    quads: &[Quad],
    new_subjects: &[NamedNode],
    store: Option<&Store>,
) -> Vec<CatalogViolation> {
    let mut out = Vec::new();
    // Collect (subject, command) pairs from mutation + store, filtered to
    // the active (non-superseded) set in the union of both.
    let mutation_superseded: BTreeSet<String> = collect_superseded_targets(quads);
    let mut active: BTreeMap<String, Vec<String>> = BTreeMap::new(); // command -> [subject]

    for s in new_subjects {
        let cmd = literal_values(quads, s, IRI_DEC_COMMAND_CR);
        let Some(c) = cmd.first() else { continue };
        let iri = s.as_str().to_string();
        if mutation_superseded.contains(&iri) {
            continue;
        }
        // Skip if mutation explicitly supersedes this new subject (rare).
        active.entry(c.clone()).or_default().push(iri);
    }

    if let Some(store) = store {
        // Pull existing CapabilityReferences from the store and add them
        // to the active map if they're not superseded (either already in
        // the store or being superseded by this mutation).
        let existing = active_capability_refs_in_store(store);
        let store_superseded = store_superseded_targets(store, "https://decision-cli.dev/ns/cr/");
        for (iri, command) in existing {
            // Skip if mutation supersedes this existing subject.
            if mutation_superseded.contains(&iri) || store_superseded.contains(&iri) {
                continue;
            }
            // Skip self if we're re-validating an existing subject already in `new_subjects`.
            if new_subjects.iter().any(|n| n.as_str() == iri) {
                continue;
            }
            active.entry(command).or_default().push(iri);
        }
    }

    for (cmd, subjects) in &active {
        if subjects.len() <= 1 {
            continue;
        }
        // Identify the existing subject (one already in the store) for
        // the diagnostic "DuplicateActive — existing: <iri>".
        let new_in_mutation: BTreeSet<&str> = new_subjects.iter().map(|n| n.as_str()).collect();
        let existing: Vec<&String> = subjects
            .iter()
            .filter(|s| !new_in_mutation.contains(s.as_str()))
            .collect();
        for subj in subjects {
            if !new_in_mutation.contains(subj.as_str()) {
                continue;
            }
            let node = NamedNode::new_unchecked(subj);
            let detail = if let Some(prior) = existing.first() {
                format!(
                    "DuplicateActive — dec:command {cmd:?} already covered by active reference <{prior}> (supersede it first)"
                )
            } else {
                format!(
                    "DuplicateActive — dec:command {cmd:?} appears on {n} active CapabilityReference subjects in this mutation",
                    n = subjects.len()
                )
            };
            out.push(violation(
                "CapabilityReferenceShape",
                &node,
                "dec:command",
                &detail,
            ));
        }
    }
    out
}

fn active_capability_refs_in_store(store: &Store) -> Vec<(String, String)> {
    let q = format!(
        "SELECT ?s ?cmd WHERE {{ \
            GRAPH <{g}> {{ \
                ?s a <{cls}> ; \
                   <{p}> ?cmd . \
                FILTER NOT EXISTS {{ ?s <{sb}> ?_ }} \
            }} \
         }}",
        g = IRI_DEC_GRAPH_CATALOG,
        cls = IRI_DEC_CAPABILITY_REFERENCE,
        p = IRI_DEC_COMMAND_CR,
        sb = SUPERSEDED_BY,
    );
    sparql_pairs(store, &q)
}

// ---------------------------------------------------------------------------
// OntologyDescription: single-active invariant (TC-166)
// ---------------------------------------------------------------------------

fn check_ontology_single_active(
    quads: &[Quad],
    new_subjects: &[NamedNode],
    store: Option<&Store>,
) -> Vec<CatalogViolation> {
    let mut out = Vec::new();
    let mutation_superseded = collect_superseded_targets(quads);
    let mut active_set: Vec<String> = Vec::new();
    for s in new_subjects {
        if mutation_superseded.contains(s.as_str()) {
            continue;
        }
        active_set.push(s.as_str().to_string());
    }
    if let Some(store) = store {
        let store_superseded = store_superseded_targets(store, "https://decision-cli.dev/ns/od/");
        for iri in active_ontology_descriptions_in_store(store) {
            if mutation_superseded.contains(&iri) || store_superseded.contains(&iri) {
                continue;
            }
            if new_subjects.iter().any(|n| n.as_str() == iri) {
                continue;
            }
            active_set.push(iri);
        }
    }
    if active_set.len() <= 1 {
        return out;
    }
    let new_in_mutation: BTreeSet<&str> = new_subjects.iter().map(|n| n.as_str()).collect();
    let existing: Vec<&String> = active_set
        .iter()
        .filter(|s| !new_in_mutation.contains(s.as_str()))
        .collect();
    for subj in &active_set {
        if !new_in_mutation.contains(subj.as_str()) {
            continue;
        }
        let node = NamedNode::new_unchecked(subj);
        let detail = if let Some(prior) = existing.first() {
            format!(
                "single-active OntologyDescription invariant: parallel active description rejected — supersede the existing active description <{prior}> first"
            )
        } else {
            format!(
                "single-active OntologyDescription invariant: {n} parallel active descriptions in mutation",
                n = active_set.len()
            )
        };
        out.push(violation(
            "OntologyDescriptionShape",
            &node,
            "dec:OntologyDescription/single-active",
            &detail,
        ));
    }
    out
}

fn active_ontology_descriptions_in_store(store: &Store) -> Vec<String> {
    let q = format!(
        "SELECT ?s WHERE {{ \
            GRAPH <{g}> {{ \
                ?s a <{cls}> . \
                FILTER NOT EXISTS {{ ?s <{sb}> ?_ }} \
            }} \
         }}",
        g = IRI_DEC_GRAPH_CATALOG,
        cls = IRI_DEC_ONTOLOGY_DESCRIPTION,
        sb = SUPERSEDED_BY,
    );
    sparql_singletons(store, &q)
}

// ---------------------------------------------------------------------------
// ExemplarGraph: backing approved VGR (TC-167)
// ---------------------------------------------------------------------------

fn check_exemplar_backed_by_approved(
    quads: &[Quad],
    subject: &NamedNode,
    store: Option<&Store>,
) -> Vec<CatalogViolation> {
    let mut out = Vec::new();
    let vg_targets = named_objects(quads, subject, IRI_DEC_EXEMPLAR_OF);
    let vgr_targets = named_objects(quads, subject, IRI_DEC_BASED_ON_APPROVED_RESULT);
    let Some(vg) = vg_targets.first() else {
        return out;
    };
    let Some(vgr) = vgr_targets.first() else {
        return out;
    };
    let Some(store) = store else {
        // No store snapshot — skip the cross-mutation invariant. The
        // store-less path is used in unit tests that need the per-field
        // shape only.
        return out;
    };
    if !vgr_is_approved_for(store, vgr, vg) {
        out.push(violation(
            "ExemplarGraphShape",
            subject,
            "dec:basedOnApprovedResult",
            &format!(
                "ExemplarNotProven — referenced VG <{vg}> has no dec:VerificationGraphResult with dec:verdict {VERDICT_APPROVED:?} and dec:resultOf <{vg}> matching the proposed dec:basedOnApprovedResult <{vgr}>"
            ),
        ));
    }
    out
}

fn vgr_is_approved_for(store: &Store, vgr: &NamedNode, vg: &NamedNode) -> bool {
    let q = format!(
        "ASK {{ \
            GRAPH ?g {{ \
                <{vgr}> <{verdict}> {verdict_lit:?} ; \
                        <{result_of}> <{vg}> . \
            }} \
         }}",
        vgr = vgr.as_str(),
        vg = vg.as_str(),
        verdict = crate::core::vocab::IRI_DEC_VERDICT,
        verdict_lit = VERDICT_APPROVED,
        result_of = crate::core::vocab::IRI_DEC_RESULT_OF,
    );
    matches!(store.query(q.as_str()), Ok(QueryResults::Boolean(true)))
}

// ---------------------------------------------------------------------------
// Supersession cycle detection (TC-165 Scenario D)
// ---------------------------------------------------------------------------

fn check_supersession_cycles(quads: &[Quad], store: Option<&Store>) -> Vec<CatalogViolation> {
    let mut edges: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    // Edges from mutation.
    for q in quads {
        if q.predicate.as_str() != SUPERSEDES {
            continue;
        }
        let Subject::NamedNode(s) = &q.subject else {
            continue;
        };
        let Term::NamedNode(t) = &q.object else {
            continue;
        };
        edges
            .entry(s.as_str().to_string())
            .or_default()
            .insert(t.as_str().to_string());
    }
    // Edges already in the store (catalog graph).
    if let Some(store) = store {
        let q = format!(
            "SELECT ?s ?t WHERE {{ GRAPH <{g}> {{ ?s <{p}> ?t }} }}",
            g = IRI_DEC_GRAPH_CATALOG,
            p = SUPERSEDES,
        );
        for (s, t) in sparql_pairs(store, &q) {
            edges.entry(s).or_default().insert(t);
        }
    }

    let mut out = Vec::new();
    for (node, _) in &edges {
        if has_cycle(node, &edges) {
            let n = NamedNode::new_unchecked(node);
            out.push(violation(
                "CapabilityReferenceShape",
                &n,
                "dec:supersedes/cycle",
                "SupersessionCycle — dec:supersedes edges form a cycle reachable from this subject",
            ));
        }
    }
    out
}

fn has_cycle(start: &str, edges: &BTreeMap<String, BTreeSet<String>>) -> bool {
    let mut visited: BTreeSet<String> = BTreeSet::new();
    let mut stack: Vec<String> = vec![start.to_string()];
    let mut depth = 0usize;
    while let Some(node) = stack.pop() {
        depth += 1;
        if depth > 1024 {
            return true;
        }
        if !visited.insert(node.clone()) {
            return node == start;
        }
        if let Some(next) = edges.get(&node) {
            for t in next {
                if t == start && node != start {
                    return true;
                }
                stack.push(t.clone());
            }
        }
    }
    // Final pass: detect simple 2-cycles A↔B that the visited-on-pop
    // walk above can miss for one-step revisits.
    if let Some(forward) = edges.get(start) {
        for t in forward {
            if let Some(back) = edges.get(t) {
                if back.contains(start) {
                    return true;
                }
            }
        }
    }
    false
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

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

fn literal_values(quads: &[Quad], subject: &NamedNode, predicate: &str) -> Vec<String> {
    let mut out = Vec::new();
    for q in quads {
        if q.predicate.as_str() != predicate {
            continue;
        }
        let Subject::NamedNode(s) = &q.subject else {
            continue;
        };
        if s != subject {
            continue;
        }
        if let Term::Literal(lit) = &q.object {
            out.push(lit.value().to_string());
        }
    }
    out
}

fn named_objects(quads: &[Quad], subject: &NamedNode, predicate: &str) -> Vec<NamedNode> {
    let mut out = Vec::new();
    for q in quads {
        if q.predicate.as_str() != predicate {
            continue;
        }
        let Subject::NamedNode(s) = &q.subject else {
            continue;
        };
        if s != subject {
            continue;
        }
        if let Term::NamedNode(n) = &q.object {
            out.push(n.clone());
        }
    }
    out
}

fn has_any_predicate(quads: &[Quad], subject: &NamedNode, predicate: &str) -> bool {
    quads.iter().any(|q| {
        q.predicate.as_str() == predicate
            && matches!(&q.subject, Subject::NamedNode(s) if s == subject)
    })
}

fn require_single_literal(
    quads: &[Quad],
    subject: &NamedNode,
    predicate: &str,
    shape: &str,
    label: &str,
    out: &mut Vec<CatalogViolation>,
) {
    let v = literal_values(quads, subject, predicate);
    if v.is_empty() {
        out.push(violation(
            shape,
            subject,
            label,
            &format!("missing required {label} (sh:minCount 1)"),
        ));
    } else if v.len() > 1 {
        out.push(violation(
            shape,
            subject,
            label,
            &format!(
                "expected exactly one {label} literal, found {n}",
                n = v.len()
            ),
        ));
    }
}

fn collect_superseded_targets(quads: &[Quad]) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for q in quads {
        if q.predicate.as_str() != SUPERSEDES {
            continue;
        }
        if let Term::NamedNode(target) = &q.object {
            out.insert(target.as_str().to_string());
        }
    }
    // Also accept supersededBy authored directly.
    for q in quads {
        if q.predicate.as_str() != SUPERSEDED_BY {
            continue;
        }
        if let Subject::NamedNode(s) = &q.subject {
            out.insert(s.as_str().to_string());
        }
    }
    out
}

fn store_superseded_targets(store: &Store, iri_prefix: &str) -> BTreeSet<String> {
    let q = format!(
        "SELECT ?old WHERE {{ \
            GRAPH <{g}> {{ \
                {{ ?new <{sup}> ?old }} UNION {{ ?old <{sb}> ?_ }} \
                FILTER(STRSTARTS(STR(?old), \"{p}\")) \
            }} \
         }}",
        g = IRI_DEC_GRAPH_CATALOG,
        sup = SUPERSEDES,
        sb = SUPERSEDED_BY,
        p = iri_prefix,
    );
    sparql_singletons(store, &q).into_iter().collect()
}

fn sparql_pairs(store: &Store, query: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    if let Ok(QueryResults::Solutions(sols)) = store.query(query) {
        for sol in sols.flatten() {
            let s = sol.get(0).and_then(term_iri).unwrap_or_default();
            let t = sol.get(1).and_then(term_str).unwrap_or_default();
            out.push((s, t));
        }
    }
    out
}

fn sparql_singletons(store: &Store, query: &str) -> Vec<String> {
    let mut out = Vec::new();
    if let Ok(QueryResults::Solutions(sols)) = store.query(query) {
        for sol in sols.flatten() {
            if let Some(s) = sol.get(0).and_then(term_iri) {
                out.push(s);
            }
        }
    }
    out
}

fn term_iri(t: &Term) -> Option<String> {
    match t {
        Term::NamedNode(n) => Some(n.as_str().to_string()),
        _ => None,
    }
}

fn term_str(t: &Term) -> Option<String> {
    match t {
        Term::NamedNode(n) => Some(n.as_str().to_string()),
        Term::Literal(l) => Some(l.value().to_string()),
        _ => None,
    }
}

fn violation(shape: &str, subject: &NamedNode, path: &str, detail: &str) -> CatalogViolation {
    CatalogViolation {
        shape: shape.to_string(),
        subject: subject.as_str().to_string(),
        path: path.to_string(),
        detail: detail.to_string(),
    }
}

fn render(violations: &[CatalogViolation]) -> String {
    use std::fmt::Write;
    let mut out = String::new();
    for v in violations {
        let _ = writeln!(
            out,
            "  • [{shape}] subject <{subj}> path {path}: {detail}",
            shape = v.shape,
            subj = v.subject,
            path = v.path,
            detail = v.detail
        );
    }
    out
}
