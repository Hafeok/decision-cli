//! SHACL-style constraint checks used during init validation.

use oxigraph::model::{NamedNode, Term};
use oxigraph::store::Store;

use super::sparql::{collect_property_values, collect_string_property, term_kind};
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
    let required = [
        (DEC_NAME, true, true),
        (DEC_TITLE, true, true),
    ];
    for (prop, is_string, single) in required {
        check_property(store, graph, subject_iri, prop, is_string, single, &mut violations);
    }

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

    let authorized = collect_string_property(store, subject_iri, DEC_AUTHORIZED_GOALS);
    if authorized.is_empty() {
        violations.push(Violation {
            target: subject_iri.to_string(),
            path: DEC_AUTHORIZED_GOALS.to_string(),
            detail: format!("missing required property {DEC_AUTHORIZED_GOALS} (sh:minCount 1)"),
        });
    }
    violations
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
        ("https://decision-cli.dev/ns#expectedOutputType", false, false),
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
        violations.push(Violation {
            target: subject_iri.to_string(),
            path: prop.to_string(),
            detail: format!("missing required property {prop} (sh:minCount 1)"),
        });
        return;
    }
    if single && values.len() > 1 {
        violations.push(Violation {
            target: subject_iri.to_string(),
            path: prop.to_string(),
            detail: format!(
                "expected exactly one {prop}, found {} (sh:maxCount 1)",
                values.len()
            ),
        });
    }
    if is_string {
        for v in &values {
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
}
