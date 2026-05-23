//! Planning capability/binding writes against the existing graph (FT-058).

use oxigraph::model::{NamedNode, Quad, Term};
use oxigraph::sparql::QueryResults;
use oxigraph::store::Store;

use crate::core::ontology::capability::types::Capability;
use crate::core::ontology::capability::{
    query_active_by_id as query_active_capability, validate_quads as validate_capability_quads,
    CapabilityShaclError,
};
use crate::core::ontology::role_binding::types::RoleBinding;
use crate::core::ontology::role_binding::{
    active_for_role, validate_quads as validate_binding_quads, RoleBindingShaclError,
};
use crate::core::vocab::{capability_graph, role_binding_graph};

use super::catalog::{BootstrapError, BootstrapReport};
use super::diff::{diff_binding, diff_capability, FieldDiff};

/// Walk every YAML-derived capability and decide skip-vs-write-vs-error.
/// Returns the set of quads to insert (empty when every entry already
/// matched the graph).
pub(super) fn plan_capability_writes(
    store: &Store,
    capabilities: &[Capability],
    report: &mut BootstrapReport,
) -> Result<Vec<Quad>, BootstrapError> {
    let mut to_write: Vec<Quad> = Vec::new();
    let cap_graph = capability_graph();
    for cap in capabilities {
        let stored = lookup_stored_capability(store, cap)?;
        match stored {
            Some(stored_cap) => {
                let diffs = diff_capability(cap, &stored_cap);
                if diffs.is_empty() {
                    report.capabilities_skipped += 1;
                } else {
                    return Err(BootstrapError::Divergence {
                        artifact: format!("{}@v{}", cap.id, cap.version),
                        diff: render_diffs(&diffs),
                        diffs,
                    });
                }
            }
            None => {
                let quads = cap.to_quads(cap_graph);
                validate_capability_quads(&quads).map_err(|e: CapabilityShaclError| {
                    BootstrapError::ShaclViolated {
                        artifact: format!("{}@v{}", cap.id, cap.version),
                        report: e.report,
                    }
                })?;
                to_write.extend(quads);
                report.capabilities_written += 1;
            }
        }
    }
    Ok(to_write)
}

/// Look up the stored capability for the YAML entry by canonical
/// `(id, version)` IRI — ignoring status. Status-aware lookup
/// (`query_active_by_id`) is unsuitable for the bootstrap because
/// `candidate`-status capabilities are skipped by that helper.
fn lookup_stored_capability(
    store: &Store,
    yaml: &Capability,
) -> Result<Option<Capability>, BootstrapError> {
    let iri = yaml.iri();
    if !subject_exists(store, &iri).map_err(BootstrapError::Internal)? {
        return Ok(None);
    }
    // The artifact exists; reuse the standard reader by matching on
    // capability_id when it happens to be active, otherwise read the
    // quads directly via the active reader's machinery is not possible —
    // so we fall back to the active-by-id reader when status == active
    // and surface a "different version" view otherwise.
    match query_active_capability(store, &yaml.id) {
        Ok(Some(c)) if c.version == yaml.version => Ok(Some(c)),
        Ok(_) | Err(_) => Ok(load_any_capability(store, &iri)),
    }
}

/// Walk the persisted store to load a capability by its canonical IRI
/// regardless of status (mirrors the reader used by the dispatcher but
/// without the active-only filter).
fn load_any_capability(store: &Store, iri: &NamedNode) -> Option<Capability> {
    use crate::core::ontology::capability::types::{CapabilityStatus, CostCurrency, Endpoint};
    use crate::core::vocab::{
        IRI_DEC_BOOTSTRAP_SOURCE, IRI_DEC_CAPABILITY_ENDPOINT, IRI_DEC_CAPABILITY_ID,
        IRI_DEC_CAPABILITY_NOTES, IRI_DEC_CAPABILITY_STATUS, IRI_DEC_CAPABILITY_VERSION,
        IRI_DEC_CONFIGURABLE_EFFORT, IRI_DEC_CONTEXT_WINDOW, IRI_DEC_COST_CACHE_HIT_PER_M,
        IRI_DEC_COST_CACHE_WRITE_5M, IRI_DEC_COST_CURRENCY, IRI_DEC_COST_INPUT_PER_M,
        IRI_DEC_COST_OUTPUT_PER_M, IRI_DEC_EXPOSES_REASONING_TRACE, IRI_DEC_MAX_OUTPUT,
        IRI_DEC_MODEL_IDENTIFIER, IRI_DEC_SUPPORTS_TOOL_CALLING, IRI_DEC_SUPPORTS_VISION,
        IRI_DEC_TIER,
    };
    let lit = |p: &str| first_literal(store, iri, p);
    let id = lit(IRI_DEC_CAPABILITY_ID)?;
    let endpoint = Endpoint::try_from_str(&lit(IRI_DEC_CAPABILITY_ENDPOINT)?)?;
    let currency = CostCurrency::try_from_str(&lit(IRI_DEC_COST_CURRENCY)?)?;
    let status = CapabilityStatus::try_from_str(&lit(IRI_DEC_CAPABILITY_STATUS)?)?;
    let cap = Capability {
        id,
        endpoint,
        model_identifier: lit(IRI_DEC_MODEL_IDENTIFIER)?,
        tier: lit(IRI_DEC_TIER).and_then(|s| s.parse().ok()),
        context_window: lit(IRI_DEC_CONTEXT_WINDOW)?.parse().ok()?,
        max_output: lit(IRI_DEC_MAX_OUTPUT)?.parse().ok()?,
        supports_vision: lit(IRI_DEC_SUPPORTS_VISION)? == "true",
        supports_tool_calling: lit(IRI_DEC_SUPPORTS_TOOL_CALLING)? == "true",
        cost_input_per_m: lit(IRI_DEC_COST_INPUT_PER_M)?,
        cost_output_per_m: lit(IRI_DEC_COST_OUTPUT_PER_M)?,
        cost_cache_hit_per_m: lit(IRI_DEC_COST_CACHE_HIT_PER_M),
        cost_cache_write_5m: lit(IRI_DEC_COST_CACHE_WRITE_5M),
        cost_currency: currency,
        configurable_effort: lit(IRI_DEC_CONFIGURABLE_EFFORT).map(|s| s == "true"),
        exposes_reasoning_trace: lit(IRI_DEC_EXPOSES_REASONING_TRACE).map(|s| s == "true"),
        status,
        version: lit(IRI_DEC_CAPABILITY_VERSION)?.parse().ok()?,
        supersedes: None,
        bootstrap_source: lit(IRI_DEC_BOOTSTRAP_SOURCE),
        notes: lit(IRI_DEC_CAPABILITY_NOTES),
    };
    Some(cap)
}

fn first_literal(store: &Store, subject: &NamedNode, predicate: &str) -> Option<String> {
    let q = format!(
        "SELECT ?v WHERE {{ \
           {{ <{s}> <{p}> ?v }} \
           UNION \
           {{ GRAPH ?g {{ <{s}> <{p}> ?v }} }} \
         }} LIMIT 1",
        s = subject.as_str(),
        p = predicate,
    );
    let QueryResults::Solutions(mut sols) = store.query(q.as_str()).ok()? else {
        return None;
    };
    let sol = sols.next()?.ok()?;
    match sol.get("v")? {
        Term::Literal(lit) => Some(lit.value().to_string()),
        Term::NamedNode(n) => Some(n.as_str().to_string()),
        _ => None,
    }
}

fn subject_exists(store: &Store, iri: &NamedNode) -> Result<bool, String> {
    let q = format!(
        "ASK {{ \
           {{ <{s}> ?p ?o }} \
           UNION \
           {{ GRAPH ?g {{ <{s}> ?p ?o }} }} \
         }}",
        s = iri.as_str(),
    );
    match store.query(q.as_str()).map_err(|e| e.to_string())? {
        QueryResults::Boolean(b) => Ok(b),
        _ => Ok(false),
    }
}

/// Walk every YAML-derived binding and decide skip-vs-write-vs-error.
pub(super) fn plan_binding_writes(
    store: &Store,
    bindings: &[RoleBinding],
    report: &mut BootstrapReport,
) -> Result<Vec<Quad>, BootstrapError> {
    let mut to_write: Vec<Quad> = Vec::new();
    let bind_graph = role_binding_graph();
    for binding in bindings {
        match active_for_role(store, &binding.role_id) {
            Ok(Some(stored)) if stored.version == binding.version => {
                let diffs = diff_binding(binding, &stored);
                if diffs.is_empty() {
                    report.bindings_skipped += 1;
                } else {
                    return Err(BootstrapError::Divergence {
                        artifact: format!("{}@v{}", binding.role_id, binding.version),
                        diff: render_diffs(&diffs),
                        diffs,
                    });
                }
            }
            Ok(Some(_)) | Ok(None) => {
                let quads = binding.to_quads(bind_graph);
                validate_binding_quads(&quads).map_err(|e: RoleBindingShaclError| {
                    BootstrapError::ShaclViolated {
                        artifact: format!("{}@v{}", binding.role_id, binding.version),
                        report: e.report,
                    }
                })?;
                to_write.extend(quads);
                report.bindings_written += 1;
            }
            Err(e) => return Err(BootstrapError::Internal(e.to_string())),
        }
    }
    Ok(to_write)
}

/// Atomically insert every quad from both lists in a single store
/// transaction. SHACL errors and partial writes never land — the
/// transaction either commits all quads or rolls back entirely.
pub(super) fn commit_all(
    store: &Store,
    cap_quads: &[Quad],
    bind_quads: &[Quad],
) -> Result<(), BootstrapError> {
    store
        .transaction(|mut tx| {
            for q in cap_quads {
                tx.insert(q.as_ref())?;
            }
            for q in bind_quads {
                tx.insert(q.as_ref())?;
            }
            Ok::<_, oxigraph::store::StorageError>(())
        })
        .map_err(|e| BootstrapError::Internal(e.to_string()))
}

fn render_diffs(diffs: &[FieldDiff]) -> String {
    let mut report = String::new();
    for d in diffs {
        let line = format!(
            "  • field `{}`: yaml={:?} graph={:?}\n",
            d.field, d.yaml_value, d.graph_value
        );
        report.push_str(&line);
    }
    report
}
