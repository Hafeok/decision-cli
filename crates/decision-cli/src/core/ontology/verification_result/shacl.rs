//! Write-side SHACL validation for `dec:VerificationGraphResult` and
//! `dec:VerificationStepTrace` (FT-097 / ADR-028).
//!
//! Enforces the four invariants declared in FT-097 §Invariants:
//!
//! 1. **Length parity** — `dec:stepTraces` count matches the parent
//!    `dec:VerificationGraph.dec:steps` count.
//! 2. **Step-IRI membership** — every `dec:tracesStep` references a step
//!    IRI that exists in the parent graph.
//! 3. **Verdict-vs-trace consistency** — the result's `dec:verdict`
//!    matches the per-graph rule re-asserted on the trace pattern.
//! 4. **Rationale `sh:minLength 20`** — matches ADR-018 verdict rationale.

use std::collections::{BTreeMap, BTreeSet};

use oxigraph::model::{NamedNode, Quad, Term};
use oxigraph::store::Store;
use thiserror::Error;

use crate::core::ontology::verdict::RATIONALE_MIN_LEN;
use crate::core::vocab::{
    IRI_DEC_OUTCOME, IRI_DEC_RATIONALE, IRI_DEC_RESULT_OF, IRI_DEC_STEPS, IRI_DEC_STEP_TRACES,
    IRI_DEC_TRACES_STEP, IRI_DEC_VERDICT, IRI_DEC_VERIFICATION_GRAPH_RESULT,
    IRI_DEC_VERIFICATION_STEP_TRACE, OUTCOME_FAIL, OUTCOME_PASS, OUTCOME_TOKENS,
    OUTCOME_UNRUNNABLE, VERDICT_AMENDMENT_REQUIRED, VERDICT_APPROVED, VERDICT_REJECTED,
};

use super::types::{RDF_FIRST, RDF_NIL, RDF_REST, RDF_TYPE};

/// One SHACL violation against a candidate result mutation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResultViolation {
    /// SHACL shape name the violation belongs to.
    pub shape: String,
    /// Subject IRI the violation is attached to.
    pub subject: String,
    /// Predicate path the violation is against.
    pub path: String,
    /// Operator-friendly explanation.
    pub detail: String,
}

/// Structured failure for SHACL validation of a result or trace.
#[derive(Debug, Error)]
#[error("SHACL validation failed for VerificationGraphResult/Trace:\n{report}")]
pub struct ResultShaclError {
    /// Rendered report.
    pub report: String,
    /// The raw violations, in input order.
    pub violations: Vec<ResultViolation>,
}

/// Run the FT-097 SHACL shapes over every result and trace subject in
/// `quads`. Cross-artifact constraints (length parity, step IRI
/// membership, verdict-vs-trace) consult `parent_graph_lookup` for the
/// parent `VerificationGraph` shape if supplied; when `None` they are
/// best-effort against information present in `quads` itself.
pub fn validate_quads(quads: &[Quad]) -> Result<(), ResultShaclError> {
    validate_quads_with_store(quads, None)
}

/// Variant that consults `store` for parent-graph triples when present.
pub fn validate_quads_with_store(
    quads: &[Quad],
    store: Option<&Store>,
) -> Result<(), ResultShaclError> {
    let mut violations = Vec::new();
    let result_subjects = result_subjects(quads);
    let trace_subjects = trace_subjects(quads);
    for subject in &result_subjects {
        violations.extend(validate_result_subject(quads, subject, store));
    }
    for subject in &trace_subjects {
        violations.extend(validate_trace_subject(quads, subject, store));
    }
    if violations.is_empty() {
        return Ok(());
    }
    Err(ResultShaclError {
        report: render_violations(&violations),
        violations,
    })
}

// ---------------------------------------------------------------------------
// VerificationGraphResult shape
// ---------------------------------------------------------------------------

fn validate_result_subject(
    quads: &[Quad],
    subject: &NamedNode,
    store: Option<&Store>,
) -> Vec<ResultViolation> {
    let mut violations = Vec::new();
    check_result_verdict(quads, subject, &mut violations);
    check_result_rationale(quads, subject, &mut violations);
    let parent_graph = check_result_of(quads, subject, &mut violations);
    let trace_iris = collect_step_trace_list(quads, subject);
    check_step_traces_length_parity(quads, subject, parent_graph.as_ref(), &trace_iris, store, &mut violations);
    check_step_trace_membership(quads, subject, parent_graph.as_ref(), &trace_iris, store, &mut violations);
    check_verdict_trace_consistency(quads, subject, &trace_iris, &mut violations);
    violations
}

fn check_result_verdict(
    quads: &[Quad],
    subject: &NamedNode,
    violations: &mut Vec<ResultViolation>,
) {
    let values = literal_values(quads, subject, IRI_DEC_VERDICT);
    if values.is_empty() {
        violations.push(result_violation(
            subject,
            IRI_DEC_VERDICT,
            "missing required dec:verdict (sh:minCount 1)",
        ));
        return;
    }
    if values.len() > 1 {
        violations.push(result_violation(
            subject,
            IRI_DEC_VERDICT,
            &format!("expected exactly one dec:verdict, found {}", values.len()),
        ));
    }
    let allowed: BTreeSet<&str> = [VERDICT_APPROVED, VERDICT_REJECTED, VERDICT_AMENDMENT_REQUIRED]
        .into_iter()
        .collect();
    for v in &values {
        if !allowed.contains(v.as_str()) {
            violations.push(result_violation(
                subject,
                IRI_DEC_VERDICT,
                &format!(
                    "dec:verdict must be one of {{approved, rejected, amendment-required}}, got {v:?}"
                ),
            ));
        }
    }
}

fn check_result_rationale(
    quads: &[Quad],
    subject: &NamedNode,
    violations: &mut Vec<ResultViolation>,
) {
    let values = literal_values(quads, subject, IRI_DEC_RATIONALE);
    if values.is_empty() {
        violations.push(result_violation(
            subject,
            IRI_DEC_RATIONALE,
            "missing required dec:rationale (sh:minCount 1)",
        ));
        return;
    }
    for v in &values {
        if v.chars().count() < RATIONALE_MIN_LEN {
            violations.push(result_violation(
                subject,
                IRI_DEC_RATIONALE,
                &format!(
                    "dec:rationale must be ≥ {RATIONALE_MIN_LEN} chars (sh:minLength), got {}",
                    v.chars().count()
                ),
            ));
        }
    }
}

fn check_result_of(
    quads: &[Quad],
    subject: &NamedNode,
    violations: &mut Vec<ResultViolation>,
) -> Option<NamedNode> {
    let values = iri_values(quads, subject, IRI_DEC_RESULT_OF);
    if values.is_empty() {
        violations.push(result_violation(
            subject,
            IRI_DEC_RESULT_OF,
            "missing required dec:resultOf (sh:minCount 1)",
        ));
        return None;
    }
    if values.len() > 1 {
        violations.push(result_violation(
            subject,
            IRI_DEC_RESULT_OF,
            &format!("expected exactly one dec:resultOf, found {}", values.len()),
        ));
    }
    Some(NamedNode::new_unchecked(&values[0]))
}

fn check_step_traces_length_parity(
    quads: &[Quad],
    subject: &NamedNode,
    parent_graph: Option<&NamedNode>,
    trace_iris: &[NamedNode],
    store: Option<&Store>,
    violations: &mut Vec<ResultViolation>,
) {
    let Some(graph) = parent_graph else {
        return;
    };
    let Some(parent_steps) = lookup_parent_steps(quads, graph, store) else {
        // Parent graph not visible — defer to runtime check downstream.
        return;
    };
    // FT-098 §Behaviour: Phase-1/Phase-2 abort paths persist a rejected
    // VGR with `dec:stepTraces rdf:nil` — the run never started, so
    // length parity is structurally inapplicable. Allow `trace_iris ==
    // 0` to coexist with a non-empty parent graph IFF the result's
    // verdict is `rejected` (signalling pre-run abort).
    if trace_iris.is_empty() && parent_steps.len() > 0 {
        let verdict_values = literal_values(quads, subject, IRI_DEC_VERDICT);
        if verdict_values.iter().any(|v| v == VERDICT_REJECTED) {
            return;
        }
    }
    if trace_iris.len() != parent_steps.len() {
        violations.push(result_violation(
            subject,
            IRI_DEC_STEP_TRACES,
            &format!(
                "dec:stepTraces length {} does not match parent VerificationGraph step count {}",
                trace_iris.len(),
                parent_steps.len()
            ),
        ));
    }
}

fn check_step_trace_membership(
    quads: &[Quad],
    _subject: &NamedNode,
    parent_graph: Option<&NamedNode>,
    trace_iris: &[NamedNode],
    store: Option<&Store>,
    violations: &mut Vec<ResultViolation>,
) {
    let Some(graph) = parent_graph else {
        return;
    };
    let Some(parent_steps) = lookup_parent_steps(quads, graph, store) else {
        return;
    };
    let parent_set: BTreeSet<String> =
        parent_steps.iter().map(|n| n.as_str().to_string()).collect();
    for trace_iri in trace_iris {
        // The traces_step value is a property of the trace itself, but
        // we can look it up either via the local quads or via the store.
        let referenced = trace_traces_step(quads, trace_iri, store);
        let Some(step_ref) = referenced else {
            // Trace's own SHACL check (validate_trace_subject) will flag.
            continue;
        };
        if !parent_set.contains(step_ref.as_str()) {
            violations.push(ResultViolation {
                shape: "VerificationStepTraceShape".into(),
                subject: trace_iri.as_str().to_string(),
                path: IRI_DEC_TRACES_STEP.into(),
                detail: format!(
                    "dec:tracesStep <{}> not present in parent VerificationGraph <{}>",
                    step_ref.as_str(),
                    graph.as_str()
                ),
            });
        }
    }
}

fn check_verdict_trace_consistency(
    quads: &[Quad],
    subject: &NamedNode,
    trace_iris: &[NamedNode],
    violations: &mut Vec<ResultViolation>,
) {
    let verdict_values = literal_values(quads, subject, IRI_DEC_VERDICT);
    if verdict_values.len() != 1 {
        return;
    }
    let declared = verdict_values[0].clone();
    let outcomes: Vec<String> = trace_iris
        .iter()
        .map(|t| trace_outcome(quads, t).unwrap_or_default())
        .collect();
    // Per FT-097 single-graph rule:
    // - all pass → approved
    // - any fail → rejected (regardless of providesEvidenceFor — SHACL
    //   cannot easily inspect the parent's providesEvidenceFor here, so
    //   the rule is "fail dominates everything that is not also fail".
    //   Tests rely on the conservative bridge: fail-anywhere ⇒ verdict
    //   must NOT be approved.)
    // - unrunnable + no fail → amendment-required
    // - empty → approved (vacuous)
    let any_fail = outcomes.iter().any(|o| o == OUTCOME_FAIL);
    let any_unrunnable = outcomes.iter().any(|o| o == OUTCOME_UNRUNNABLE);
    let all_pass = !outcomes.is_empty() && outcomes.iter().all(|o| o == OUTCOME_PASS);
    // FT-098 §Behaviour: an empty trace list signals a Phase-1/Phase-2
    // abort (safety violation, env setup failure). The declared verdict
    // is authoritative in that case — the run never produced any
    // outcomes to reconcile, so no contradiction is possible.
    if outcomes.is_empty() {
        return;
    }
    let inferred = if outcomes.is_empty() {
        VERDICT_APPROVED
    } else if all_pass {
        VERDICT_APPROVED
    } else if any_fail {
        VERDICT_REJECTED
    } else if any_unrunnable {
        VERDICT_AMENDMENT_REQUIRED
    } else {
        VERDICT_REJECTED
    };
    let consistent = declared == inferred
        // Conservative tie-break: a `rejected` verdict is permitted even
        // when the inferred per-rule answer is `amendment-required` —
        // FT-097 lets the runner classify mixed unrunnable/setup-style
        // failures as either. The hard rule we enforce is "the SHACL
        // shape rejects results whose verdict contradicts the trace
        // pattern in a way that cannot be reconciled" — i.e. approved
        // verdicts paired with any non-pass trace.
        || (declared != VERDICT_APPROVED && inferred != VERDICT_APPROVED);
    if !consistent {
        violations.push(result_violation(
            subject,
            IRI_DEC_VERDICT,
            &format!(
                "dec:verdict {:?} contradicts the step-trace pattern (inferred {:?} from outcomes {:?})",
                declared, inferred, outcomes
            ),
        ));
    }
}

// ---------------------------------------------------------------------------
// VerificationStepTrace shape
// ---------------------------------------------------------------------------

fn validate_trace_subject(
    quads: &[Quad],
    subject: &NamedNode,
    _store: Option<&Store>,
) -> Vec<ResultViolation> {
    let mut violations = Vec::new();
    // tracesStep — required, exactly one IRI
    let values = iri_values(quads, subject, IRI_DEC_TRACES_STEP);
    if values.is_empty() {
        violations.push(trace_violation(
            subject,
            IRI_DEC_TRACES_STEP,
            "missing required dec:tracesStep (sh:minCount 1)",
        ));
    } else if values.len() > 1 {
        violations.push(trace_violation(
            subject,
            IRI_DEC_TRACES_STEP,
            &format!("expected exactly one dec:tracesStep, found {}", values.len()),
        ));
    }
    // outcome — required, allowed enum
    let outcomes = literal_values(quads, subject, IRI_DEC_OUTCOME);
    if outcomes.is_empty() {
        violations.push(trace_violation(
            subject,
            IRI_DEC_OUTCOME,
            "missing required dec:outcome (sh:minCount 1)",
        ));
    } else {
        let allowed: BTreeSet<&str> = OUTCOME_TOKENS.iter().copied().collect();
        for v in &outcomes {
            if !allowed.contains(v.as_str()) {
                violations.push(trace_violation(
                    subject,
                    IRI_DEC_OUTCOME,
                    &format!(
                        "dec:outcome must be one of {{pass, fail, unrunnable}}, got {v:?}"
                    ),
                ));
            }
        }
    }
    violations
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn result_subjects(quads: &[Quad]) -> Vec<NamedNode> {
    type_subjects(quads, IRI_DEC_VERIFICATION_GRAPH_RESULT)
}

fn trace_subjects(quads: &[Quad]) -> Vec<NamedNode> {
    type_subjects(quads, IRI_DEC_VERIFICATION_STEP_TRACE)
}

fn type_subjects(quads: &[Quad], class_iri: &str) -> Vec<NamedNode> {
    let mut out = Vec::new();
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
        if let oxigraph::model::Subject::NamedNode(s) = &q.subject {
            if !out.iter().any(|n| n == s) {
                out.push(s.clone());
            }
        }
    }
    out
}

fn collect_step_trace_list(quads: &[Quad], subject: &NamedNode) -> Vec<NamedNode> {
    // Find the head term for dec:stepTraces, then walk rdf:first/rdf:rest.
    let mut heads: Vec<Term> = Vec::new();
    for q in quads {
        if q.predicate.as_str() != IRI_DEC_STEP_TRACES {
            continue;
        }
        if !subject_matches(q, subject) {
            continue;
        }
        heads.push(q.object.clone());
    }
    if heads.len() != 1 {
        return Vec::new();
    }
    let head = heads.remove(0);
    walk_list(quads, &head)
}

fn walk_list(quads: &[Quad], head: &Term) -> Vec<NamedNode> {
    let mut out = Vec::new();
    let mut current = head.clone();
    let mut seen: BTreeSet<String> = BTreeSet::new();
    loop {
        if matches!(&current, Term::NamedNode(n) if n.as_str() == RDF_NIL) {
            break;
        }
        let key = node_key(&current);
        if !seen.insert(key) {
            break;
        }
        let Some(first) = list_first(quads, &current) else {
            break;
        };
        out.push(first);
        let Some(rest) = list_rest(quads, &current) else {
            break;
        };
        current = rest;
    }
    out
}

fn list_first(quads: &[Quad], head: &Term) -> Option<NamedNode> {
    for q in quads {
        if q.predicate.as_str() != RDF_FIRST {
            continue;
        }
        if !head_matches(q, head) {
            continue;
        }
        if let Term::NamedNode(n) = &q.object {
            return Some(n.clone());
        }
    }
    None
}

fn list_rest(quads: &[Quad], head: &Term) -> Option<Term> {
    for q in quads {
        if q.predicate.as_str() != RDF_REST {
            continue;
        }
        if !head_matches(q, head) {
            continue;
        }
        return Some(q.object.clone());
    }
    None
}

fn head_matches(q: &Quad, head: &Term) -> bool {
    match (head, &q.subject) {
        (Term::NamedNode(h), oxigraph::model::Subject::NamedNode(s)) => h == s,
        (Term::BlankNode(h), oxigraph::model::Subject::BlankNode(s)) => h == s,
        _ => false,
    }
}

fn node_key(t: &Term) -> String {
    match t {
        Term::NamedNode(n) => format!("iri:{}", n.as_str()),
        Term::BlankNode(b) => format!("bn:{}", b.as_str()),
        _ => "_".into(),
    }
}

fn lookup_parent_steps(
    quads: &[Quad],
    parent_graph: &NamedNode,
    store: Option<&Store>,
) -> Option<Vec<NamedNode>> {
    // First look in `quads`. If the parent graph isn't being mutated in
    // the same transaction we'll need to consult the store.
    let heads: Vec<Term> = quads
        .iter()
        .filter_map(|q| {
            if q.predicate.as_str() != IRI_DEC_STEPS {
                return None;
            }
            if !subject_matches(q, parent_graph) {
                return None;
            }
            Some(q.object.clone())
        })
        .collect();
    if !heads.is_empty() && heads.len() == 1 {
        return Some(walk_list(quads, &heads[0]));
    }
    let store = store?;
    // SPARQL CONSTRUCT-style walk via the store. We use quads_for_pattern.
    let steps_pred = NamedNode::new_unchecked(IRI_DEC_STEPS);
    let mut heads: Vec<Term> = Vec::new();
    for q in store
        .quads_for_pattern(
            Some(oxigraph::model::Subject::NamedNode(parent_graph.clone()).as_ref()),
            Some(steps_pred.as_ref()),
            None,
            None,
        )
        .filter_map(Result::ok)
    {
        heads.push(q.object);
    }
    if heads.len() != 1 {
        return None;
    }
    Some(walk_store_list(store, &heads[0]))
}

fn walk_store_list(store: &Store, head: &Term) -> Vec<NamedNode> {
    let mut out = Vec::new();
    let mut current = head.clone();
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let rdf_first = NamedNode::new_unchecked(RDF_FIRST);
    let rdf_rest = NamedNode::new_unchecked(RDF_REST);
    loop {
        if matches!(&current, Term::NamedNode(n) if n.as_str() == RDF_NIL) {
            break;
        }
        let key = node_key(&current);
        if !seen.insert(key) {
            break;
        }
        let head_subj = match &current {
            Term::NamedNode(n) => oxigraph::model::Subject::NamedNode(n.clone()),
            Term::BlankNode(b) => oxigraph::model::Subject::BlankNode(b.clone()),
            _ => break,
        };
        let mut first: Option<NamedNode> = None;
        for q in store
            .quads_for_pattern(Some(head_subj.as_ref()), Some(rdf_first.as_ref()), None, None)
            .filter_map(Result::ok)
        {
            if let Term::NamedNode(n) = q.object {
                first = Some(n);
                break;
            }
        }
        let Some(f) = first else { break };
        out.push(f);
        let mut rest: Option<Term> = None;
        for q in store
            .quads_for_pattern(Some(head_subj.as_ref()), Some(rdf_rest.as_ref()), None, None)
            .filter_map(Result::ok)
        {
            rest = Some(q.object);
            break;
        }
        let Some(r) = rest else { break };
        current = r;
    }
    out
}

fn trace_traces_step(
    quads: &[Quad],
    trace_iri: &NamedNode,
    store: Option<&Store>,
) -> Option<NamedNode> {
    // First look in `quads`.
    let in_quads: Vec<String> = iri_values(quads, trace_iri, IRI_DEC_TRACES_STEP);
    if let Some(v) = in_quads.into_iter().next() {
        return Some(NamedNode::new_unchecked(&v));
    }
    let store = store?;
    let pred = NamedNode::new_unchecked(IRI_DEC_TRACES_STEP);
    for q in store
        .quads_for_pattern(
            Some(oxigraph::model::Subject::NamedNode(trace_iri.clone()).as_ref()),
            Some(pred.as_ref()),
            None,
            None,
        )
        .filter_map(Result::ok)
    {
        if let Term::NamedNode(n) = q.object {
            return Some(n);
        }
    }
    None
}

fn trace_outcome(quads: &[Quad], trace_iri: &NamedNode) -> Option<String> {
    literal_values(quads, trace_iri, IRI_DEC_OUTCOME)
        .into_iter()
        .next()
}

fn literal_values(quads: &[Quad], subject: &NamedNode, predicate: &str) -> Vec<String> {
    quads
        .iter()
        .filter_map(|q| {
            if q.predicate.as_str() != predicate {
                return None;
            }
            if !subject_matches(q, subject) {
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
            if !subject_matches(q, subject) {
                return None;
            }
            match &q.object {
                Term::NamedNode(n) => Some(n.as_str().to_string()),
                _ => None,
            }
        })
        .collect()
}

fn subject_matches(q: &Quad, subject: &NamedNode) -> bool {
    matches!(&q.subject, oxigraph::model::Subject::NamedNode(s) if s == subject)
}

fn result_violation(subject: &NamedNode, path: &str, detail: &str) -> ResultViolation {
    ResultViolation {
        shape: "VerificationGraphResultShape".into(),
        subject: subject.as_str().to_string(),
        path: path.to_string(),
        detail: detail.to_string(),
    }
}

fn trace_violation(subject: &NamedNode, path: &str, detail: &str) -> ResultViolation {
    ResultViolation {
        shape: "VerificationStepTraceShape".into(),
        subject: subject.as_str().to_string(),
        path: path.to_string(),
        detail: detail.to_string(),
    }
}

fn render_violations(violations: &[ResultViolation]) -> String {
    use std::fmt::Write;
    let mut out = String::new();
    // Group by shape for readability.
    let mut grouped: BTreeMap<&str, Vec<&ResultViolation>> = BTreeMap::new();
    for v in violations {
        grouped.entry(v.shape.as_str()).or_default().push(v);
    }
    for (shape, vs) in grouped {
        let _ = writeln!(out, "  shape <{shape}>:");
        for v in vs {
            let _ = writeln!(
                out,
                "    • subject <{}> path <{}>: {}",
                v.subject, v.path, v.detail
            );
        }
    }
    out
}
