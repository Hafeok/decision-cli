//! SHACL-style constraint checks used during init validation.

use oxigraph::model::{NamedNode, Term};
use oxigraph::store::Store;

use super::sparql::{collect_property_values, term_kind};
use super::vocab::{
    DEC_AUTHORIZED_GOALS, DEC_COMPATIBLE_GOALS, DEC_DESCRIPTION, DEC_NAME,
    DEC_TERMINAL_VALUE_ACTION, DEC_TITLE,
};

#[derive(Debug)]
pub(super) struct Violation {
    pub target: String,
    pub path: String,
    pub detail: String,
}

pub(super) fn render_violations(vs: &[Violation]) -> String {
    let mut out = String::new();
    for v in vs {
        out.push_str(&format!(
            "  • subject <{}> path <{}>: {}\n",
            v.target, v.path, v.detail
        ));
    }
    out
}

pub(super) fn check_stream_shacl(
    store: &Store,
    graph: &NamedNode,
    subject_iri: &str,
) -> Vec<Violation> {
    let mut violations = Vec::new();
    for (prop, is_string, single) in [(DEC_NAME, true, true), (DEC_TITLE, true, true)] {
        check_property(
            store,
            graph,
            subject_iri,
            prop,
            is_string,
            single,
            &mut violations,
        );
    }
    check_terminal_value_action(store, graph, subject_iri, &mut violations);
    check_authorized_goals_present(store, graph, subject_iri, &mut violations);
    violations
}

fn check_terminal_value_action(
    store: &Store,
    graph: &NamedNode,
    subject_iri: &str,
    violations: &mut Vec<Violation>,
) {
    let tvalues = collect_property_values(store, graph, subject_iri, DEC_TERMINAL_VALUE_ACTION);
    if tvalues.is_empty() {
        violations.push(Violation {
            target: subject_iri.to_string(),
            path: DEC_TERMINAL_VALUE_ACTION.to_string(),
            detail: format!(
                "missing required property {DEC_TERMINAL_VALUE_ACTION} (sh:minCount 1)"
            ),
        });
    } else if tvalues.len() > 1 {
        violations.push(Violation {
            target: subject_iri.to_string(),
            path: DEC_TERMINAL_VALUE_ACTION.to_string(),
            detail: format!(
                "expected exactly one {DEC_TERMINAL_VALUE_ACTION}, found {}",
                tvalues.len()
            ),
        });
    } else if !matches!(&tvalues[0], Term::NamedNode(_)) {
        violations.push(Violation {
            target: subject_iri.to_string(),
            path: DEC_TERMINAL_VALUE_ACTION.to_string(),
            detail: format!("{DEC_TERMINAL_VALUE_ACTION} must be an IRI (sh:nodeKind sh:IRI)"),
        });
    }
}

fn check_authorized_goals_present(
    store: &Store,
    graph: &NamedNode,
    subject_iri: &str,
    violations: &mut Vec<Violation>,
) {
    let values = collect_property_values(store, graph, subject_iri, DEC_AUTHORIZED_GOALS);
    if values.is_empty() {
        violations.push(Violation {
            target: subject_iri.to_string(),
            path: DEC_AUTHORIZED_GOALS.to_string(),
            detail: format!("missing required property {DEC_AUTHORIZED_GOALS} (sh:minCount 1)"),
        });
    }
}

pub(super) fn check_value_action_shacl(
    store: &Store,
    graph: &NamedNode,
    subject_iri: &str,
) -> Vec<Violation> {
    let mut violations = Vec::new();
    for (prop, must_have_string, single) in [
        (DEC_NAME, true, true),
        (DEC_DESCRIPTION, true, false),
        (DEC_COMPATIBLE_GOALS, false, false),
        ("https://decision-cli.dev/ns#exitCriterion", false, false),
        (
            "https://decision-cli.dev/ns#expectedOutputType",
            false,
            false,
        ),
    ] {
        check_property(
            store,
            graph,
            subject_iri,
            prop,
            must_have_string,
            single,
            &mut violations,
        );
    }
    violations
}

#[allow(clippy::too_many_arguments)]
fn check_property(
    store: &Store,
    graph: &NamedNode,
    subject_iri: &str,
    prop: &str,
    is_string: bool,
    single: bool,
    violations: &mut Vec<Violation>,
) {
    let values = collect_property_values(store, graph, subject_iri, prop);
    if values.is_empty() {
        violations.push(missing_property_violation(subject_iri, prop));
        return;
    }
    if single && values.len() > 1 {
        violations.push(too_many_values_violation(subject_iri, prop, values.len()));
    }
    if is_string {
        append_non_literal_violations(subject_iri, prop, &values, violations);
    }
}

fn missing_property_violation(subject_iri: &str, prop: &str) -> Violation {
    Violation {
        target: subject_iri.to_string(),
        path: prop.to_string(),
        detail: format!("missing required property {prop} (sh:minCount 1)"),
    }
}

fn too_many_values_violation(subject_iri: &str, prop: &str, count: usize) -> Violation {
    Violation {
        target: subject_iri.to_string(),
        path: prop.to_string(),
        detail: format!("expected exactly one {prop}, found {count} (sh:maxCount 1)"),
    }
}

fn append_non_literal_violations(
    subject_iri: &str,
    prop: &str,
    values: &[Term],
    violations: &mut Vec<Violation>,
) {
    for v in values {
        if !matches!(v, Term::Literal(_)) {
            violations.push(Violation {
                target: subject_iri.to_string(),
                path: prop.to_string(),
                detail: format!(
                    "{prop} must be a literal (sh:datatype xsd:string), got {}",
                    term_kind(v)
                ),
            });
        }
    }
}
