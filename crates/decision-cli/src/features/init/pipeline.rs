//! Staging + validation + orchestration-store assembly for `dec init`.

use std::collections::BTreeSet;

use oxigraph::model::{GraphName, Literal, NamedNode, Quad};
use oxigraph::store::Store;

use crate::core::bundled;

use super::parse::parse_into_graph;
use super::persist::{build_session_quads, copy_triples_default, seed_bootstrap_subscriptions};
use super::shacl::{check_stream_shacl, check_value_action_shacl, render_violations};
use super::sparql::{collect_string_property, single_iri_value, sole_subject_with_class};
use super::vocab::{
    DEC_AUTHORIZED_GOALS, DEC_COMPATIBLE_GOALS, DEC_TERMINAL_VALUE_ACTION, DEC_VALUE_STREAM_CLASS,
    PROV_DERIVED_FROM, PROV_GENERATED_BY,
};
use super::{InitError, BOOTSTRAP_SESSION_IRI};

pub(super) struct StagedDefinition {
    pub staging: Store,
    pub stream_graph: NamedNode,
    pub stream_iri: String,
    pub va_graph: NamedNode,
    pub terminal_iri: String,
    pub authorized: Vec<String>,
}

pub(super) fn stage_and_validate(
    definition_bytes: &[u8],
    source_label: &str,
    base_iri: Option<&str>,
) -> Result<StagedDefinition, InitError> {
    let staging = Store::new().map_err(|e| InitError::Internal(e.to_string()))?;
    let stream_graph = NamedNode::new_unchecked("urn:decision-cli:staging:stream");
    parse_into_graph(
        &staging,
        definition_bytes,
        &stream_graph,
        source_label,
        base_iri,
    )?;

    let stream_iri = resolve_stream_iri(&staging, &stream_graph, source_label)?;
    validate_stream_shape(&staging, &stream_graph, &stream_iri, source_label)?;

    let terminal_iri = resolve_terminal_iri(&staging, &stream_iri, source_label)?;
    let va_graph = stage_bundled_value_action(&staging, &terminal_iri, source_label)?;
    validate_value_action_shape(&staging, &va_graph, &terminal_iri)?;

    let authorized = collect_authorized_goals(&staging, &stream_iri, &terminal_iri)?;

    Ok(StagedDefinition {
        staging,
        stream_graph,
        stream_iri,
        va_graph,
        terminal_iri,
        authorized,
    })
}

fn resolve_stream_iri(
    staging: &Store,
    stream_graph: &NamedNode,
    source_label: &str,
) -> Result<String, InitError> {
    sole_subject_with_class(staging, stream_graph, DEC_VALUE_STREAM_CLASS).ok_or_else(|| {
        InitError::ShaclViolation {
            source_label: source_label.to_string(),
            report: "definition does not declare any dec:ValueStream instance".to_string(),
        }
    })
}

fn validate_stream_shape(
    staging: &Store,
    stream_graph: &NamedNode,
    stream_iri: &str,
    source_label: &str,
) -> Result<(), InitError> {
    let violations = check_stream_shacl(staging, stream_graph, stream_iri);
    if !violations.is_empty() {
        return Err(InitError::ShaclViolation {
            source_label: source_label.to_string(),
            report: render_violations(&violations),
        });
    }
    Ok(())
}

fn resolve_terminal_iri(
    staging: &Store,
    stream_iri: &str,
    source_label: &str,
) -> Result<String, InitError> {
    single_iri_value(staging, stream_iri, DEC_TERMINAL_VALUE_ACTION).map_err(|detail| {
        InitError::ShaclViolation {
            source_label: source_label.to_string(),
            report: detail,
        }
    })
}

fn stage_bundled_value_action(
    staging: &Store,
    terminal_iri: &str,
    _source_label: &str,
) -> Result<NamedNode, InitError> {
    let bundled_va = bundled::lookup_value_action(terminal_iri).ok_or_else(|| {
        InitError::UnknownValueAction {
            iri: terminal_iri.to_string(),
            available: bundled::VALUE_ACTIONS
                .iter()
                .map(|v| v.iri.to_string())
                .collect::<Vec<_>>()
                .join(", "),
        }
    })?;
    let va_graph = NamedNode::new_unchecked("urn:decision-cli:staging:value-action");
    parse_into_graph(
        staging,
        bundled_va.ttl.as_bytes(),
        &va_graph,
        &format!("bundled:{}", bundled_va.iri),
        Some(bundled_va.iri),
    )?;
    Ok(va_graph)
}

fn validate_value_action_shape(
    staging: &Store,
    va_graph: &NamedNode,
    terminal_iri: &str,
) -> Result<(), InitError> {
    let violations = check_value_action_shacl(staging, va_graph, terminal_iri);
    if !violations.is_empty() {
        return Err(InitError::ShaclViolation {
            source_label: format!("bundled:{terminal_iri}"),
            report: render_violations(&violations),
        });
    }
    Ok(())
}

fn collect_authorized_goals(
    staging: &Store,
    stream_iri: &str,
    terminal_iri: &str,
) -> Result<Vec<String>, InitError> {
    let authorized: Vec<String> =
        collect_string_property(staging, stream_iri, DEC_AUTHORIZED_GOALS);
    let compatible: Vec<String> =
        collect_string_property(staging, terminal_iri, DEC_COMPATIBLE_GOALS);
    let compatible_set: BTreeSet<&str> = compatible.iter().map(String::as_str).collect();
    for goal in &authorized {
        if !compatible_set.contains(goal.as_str()) {
            return Err(InitError::UnauthorizedGoal {
                goal: goal.clone(),
                value_action: terminal_iri.to_string(),
                compatible: compatible.join(", "),
            });
        }
    }
    Ok(authorized)
}

pub(super) fn build_orchestration_store(
    staged: &StagedDefinition,
    source_label: &str,
    definition_hash: &str,
    ontology_version: &str,
    form: &str,
    now: &str,
) -> Result<Store, InitError> {
    let orchestration = Store::new().map_err(|e| InitError::Internal(e.to_string()))?;
    let dest_graph = GraphName::DefaultGraph;

    copy_staging_graphs(staged, &orchestration, &dest_graph)?;

    let session_iri =
        NamedNode::new(BOOTSTRAP_SESSION_IRI).map_err(|e| InitError::Internal(e.to_string()))?;
    insert_session_quads(
        &orchestration,
        &session_iri,
        &dest_graph,
        source_label,
        definition_hash,
        ontology_version,
        form,
        now,
    )?;

    insert_provenance_quads(
        &orchestration,
        staged,
        &session_iri,
        source_label,
        &dest_graph,
    )?;

    seed_bootstrap_subscriptions(&orchestration).map_err(|e| InitError::Internal(e.to_string()))?;

    Ok(orchestration)
}

fn copy_staging_graphs(
    staged: &StagedDefinition,
    orchestration: &Store,
    dest_graph: &GraphName,
) -> Result<(), InitError> {
    copy_triples_default(
        &staged.staging,
        orchestration,
        &staged.stream_graph,
        dest_graph,
    )?;
    copy_triples_default(&staged.staging, orchestration, &staged.va_graph, dest_graph)?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn insert_session_quads(
    orchestration: &Store,
    session_iri: &NamedNode,
    dest_graph: &GraphName,
    source_label: &str,
    definition_hash: &str,
    ontology_version: &str,
    form: &str,
    now: &str,
) -> Result<(), InitError> {
    let session_quads = build_session_quads(
        session_iri,
        dest_graph,
        source_label,
        definition_hash,
        ontology_version,
        form,
        now,
    );
    orchestration
        .transaction(|mut tx| {
            for q in &session_quads {
                tx.insert(q.as_ref())?;
            }
            Ok::<_, oxigraph::store::StorageError>(())
        })
        .map_err(|e| InitError::Internal(e.to_string()))
}

fn insert_provenance_quads(
    orchestration: &Store,
    staged: &StagedDefinition,
    session_iri: &NamedNode,
    source_label: &str,
    dest_graph: &GraphName,
) -> Result<(), InitError> {
    let quads = build_provenance_quads(staged, session_iri, source_label, dest_graph)?;
    orchestration
        .transaction(|mut tx| {
            for q in &quads {
                tx.insert(q.as_ref())?;
            }
            Ok::<_, oxigraph::store::StorageError>(())
        })
        .map_err(|e| InitError::Internal(e.to_string()))
}

fn build_provenance_quads(
    staged: &StagedDefinition,
    session_iri: &NamedNode,
    source_label: &str,
    dest_graph: &GraphName,
) -> Result<Vec<Quad>, InitError> {
    let stream_node =
        NamedNode::new(&staged.stream_iri).map_err(|e| InitError::Internal(e.to_string()))?;
    let va_node =
        NamedNode::new(&staged.terminal_iri).map_err(|e| InitError::Internal(e.to_string()))?;
    let prov_gen =
        NamedNode::new(PROV_GENERATED_BY).map_err(|e| InitError::Internal(e.to_string()))?;
    let prov_derived =
        NamedNode::new(PROV_DERIVED_FROM).map_err(|e| InitError::Internal(e.to_string()))?;
    Ok(vec![
        Quad::new(
            stream_node.clone(),
            prov_gen.clone(),
            session_iri.clone(),
            dest_graph.clone(),
        ),
        Quad::new(va_node, prov_gen, session_iri.clone(), dest_graph.clone()),
        Quad::new(
            stream_node,
            prov_derived,
            Literal::new_simple_literal(source_label),
            dest_graph.clone(),
        ),
    ])
}
