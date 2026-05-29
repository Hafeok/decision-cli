//! SPARQL read API for `dec:RoleBinding` (FT-055 §Behaviour, step 4).
//!
//! Entry points:
//!
//! - [`active_for_role`] — resolves the dispatcher's authoritative
//!   binding for a given `role_id` (the unique `dec:active = true`
//!   binding), preserving rdf:List order on the escalation chain.
//! - [`list_all_active`] — enumerates every active binding (for the
//!   forthcoming `dec binding list` command).
//! - [`all_for_role`] — enumerates all bindings (active and inactive)
//!   for a given `role_id` (FT-118).

use oxigraph::model::{NamedNode, Quad, Subject, Term};
use oxigraph::sparql::QueryResults;
use oxigraph::store::Store;
use thiserror::Error;

use crate::core::vocab::{
    IRI_DEC_BOOTSTRAP_SOURCE, IRI_DEC_CAPABILITY_SUPERSEDES, IRI_DEC_CAPABILITY_VERSION,
    IRI_DEC_DEFAULT_CAPABILITY, IRI_DEC_ESCALATION_STEPS, IRI_DEC_ROLE_BINDING,
    IRI_DEC_ROLE_BINDING_ACTIVE, IRI_DEC_ROLE_BINDING_ROLE_ID, IRI_DEC_STEP_CAPABILITY,
    IRI_DEC_TRIGGERS, IRI_DEC_TRIGGER_SIGNAL,
};

use super::read_helpers::{
    format_sparql_string, take_one_bool, take_one_iri, take_one_str, take_one_u32,
    take_optional_iri, take_optional_str,
};
use super::types::{EscalationStep, RoleBinding, TriggerSignal, RDF_FIRST, RDF_NIL, RDF_REST};

const RDF_TYPE: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#type";

/// Errors produced by the role-binding readers.
#[derive(Debug, Error)]
pub enum RoleBindingReadError {
    /// SPARQL query against the store failed.
    #[error("oxigraph store error: {0}")]
    Store(String),
    /// A persisted binding did not match the FT-055 schema.
    #[error("malformed role binding <{iri}>: {detail}")]
    Malformed {
        /// IRI of the offending binding subject.
        iri: String,
        /// Why parsing failed.
        detail: String,
    },
}

/// Return the unique active `dec:RoleBinding` whose `dec:role_id`
/// matches `role_id`. Returns `None` when no active binding exists.
pub fn active_for_role(
    store: &Store,
    role_id: &str,
) -> Result<Option<RoleBinding>, RoleBindingReadError> {
    let iris = find_active_iris(store, Some(role_id))?;
    match iris.len() {
        0 => Ok(None),
        1 => {
            let iri = iris.into_iter().next().expect("len() == 1");
            Ok(Some(load_binding(store, &iri)?))
        }
        n => Err(RoleBindingReadError::Malformed {
            iri: format!("dec:role_id={role_id:?}"),
            detail: format!(
                "{n} active role bindings share the same role_id (uniqueness invariant violated)"
            ),
        }),
    }
}

/// Enumerate every active `dec:RoleBinding` in the store. Ordering is
/// stable by `role_id` ascending (so callers can render a deterministic
/// table).
pub fn list_all_active(store: &Store) -> Result<Vec<RoleBinding>, RoleBindingReadError> {
    let iris = find_active_iris(store, None)?;
    let mut out = Vec::with_capacity(iris.len());
    for iri in iris {
        out.push(load_binding(store, &iri)?);
    }
    out.sort_by(|a, b| a.role_id.cmp(&b.role_id));
    Ok(out)
}

/// Enumerate all `dec:RoleBinding` artifacts (active and inactive) for
/// a given `role_id`. Ordering is stable by version descending (newest
/// first). FT-118 deactivation logic uses this to find prior bindings.
pub fn all_for_role(
    store: &Store,
    role_id: &str,
) -> Result<Vec<RoleBinding>, RoleBindingReadError> {
    let iris = find_all_iris(store, role_id)?;
    let mut out = Vec::with_capacity(iris.len());
    for iri in iris {
        out.push(load_binding(store, &iri)?);
    }
    out.sort_by(|a, b| b.version.cmp(&a.version));
    Ok(out)
}

fn find_all_iris(
    store: &Store,
    role_id: &str,
) -> Result<Vec<NamedNode>, RoleBindingReadError> {
    let filter_clause = format!("?b dec:role_id {lit} . ", lit = format_sparql_string(role_id));
    let q = format!(
        "PREFIX dec: <https://decision-cli.dev/ns#> \
         SELECT DISTINCT ?b WHERE {{ \
           {{ ?b a dec:RoleBinding . {filter_clause} }} \
           UNION \
           {{ GRAPH ?g {{ ?b a dec:RoleBinding . {filter_clause} }} }} \
         }}",
    );
    let QueryResults::Solutions(sols) = store
        .query(q.as_str())
        .map_err(|e| RoleBindingReadError::Store(e.to_string()))?
    else {
        return Ok(Vec::new());
    };
    let mut out: Vec<NamedNode> = Vec::new();
    for sol in sols {
        let sol = sol.map_err(|e| RoleBindingReadError::Store(e.to_string()))?;
        if let Some(Term::NamedNode(n)) = sol.get("b") {
            if !out.iter().any(|m| m == n) {
                out.push(n.clone());
            }
        }
    }
    Ok(out)
}

fn find_active_iris(
    store: &Store,
    role_filter: Option<&str>,
) -> Result<Vec<NamedNode>, RoleBindingReadError> {
    let filter_clause = match role_filter {
        Some(r) => format!("?b dec:role_id {lit} . ", lit = format_sparql_string(r)),
        None => String::new(),
    };
    let q = format!(
        "PREFIX dec: <https://decision-cli.dev/ns#> \
         SELECT DISTINCT ?b WHERE {{ \
           {{ ?b a dec:RoleBinding ; dec:active true . {filter_clause} }} \
           UNION \
           {{ GRAPH ?g {{ ?b a dec:RoleBinding ; dec:active true . {filter_clause} }} }} \
         }}",
    );
    let QueryResults::Solutions(sols) = store
        .query(q.as_str())
        .map_err(|e| RoleBindingReadError::Store(e.to_string()))?
    else {
        return Ok(Vec::new());
    };
    let mut out: Vec<NamedNode> = Vec::new();
    for sol in sols {
        let sol = sol.map_err(|e| RoleBindingReadError::Store(e.to_string()))?;
        if let Some(Term::NamedNode(n)) = sol.get("b") {
            if !out.iter().any(|m| m == n) {
                out.push(n.clone());
            }
        }
    }
    Ok(out)
}

fn load_binding(store: &Store, iri: &NamedNode) -> Result<RoleBinding, RoleBindingReadError> {
    let quads = collect_subject_quads(store, iri)?;
    require_type(iri, &quads)?;
    let role_id = take_one_str(iri, &quads, iri, IRI_DEC_ROLE_BINDING_ROLE_ID, "dec:role_id")?;
    let default_capability = take_one_iri(
        iri,
        &quads,
        iri,
        IRI_DEC_DEFAULT_CAPABILITY,
        "dec:default_capability",
    )?;
    let active = take_one_bool(iri, &quads, iri, IRI_DEC_ROLE_BINDING_ACTIVE, "dec:active")?;
    let version = take_one_u32(iri, &quads, iri, IRI_DEC_CAPABILITY_VERSION, "dec:version")?;
    let supersedes = take_optional_iri(iri, &quads, iri, IRI_DEC_CAPABILITY_SUPERSEDES)?;
    let bootstrap_source = take_optional_str(iri, &quads, iri, IRI_DEC_BOOTSTRAP_SOURCE)?;
    let escalation_steps = read_escalation_chain(store, iri)?;
    Ok(RoleBinding {
        role_id,
        default_capability,
        escalation_steps,
        version,
        active,
        supersedes,
        bootstrap_source,
    })
}

fn read_escalation_chain(
    store: &Store,
    binding: &NamedNode,
) -> Result<Vec<EscalationStep>, RoleBindingReadError> {
    let head = first_object_iri(store, binding, IRI_DEC_ESCALATION_STEPS)?;
    let Some(head) = head else {
        return Ok(Vec::new());
    };
    if head.as_str() == RDF_NIL {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    let mut cursor = head;
    loop {
        let step_iri = first_object_iri(store, &cursor, RDF_FIRST)?.ok_or_else(|| {
            RoleBindingReadError::Malformed {
                iri: binding.as_str().to_string(),
                detail: format!("rdf:List cell <{cursor}> missing rdf:first"),
            }
        })?;
        let step = load_step(store, &step_iri)?;
        out.push(step);
        let rest = first_object_iri(store, &cursor, RDF_REST)?.ok_or_else(|| {
            RoleBindingReadError::Malformed {
                iri: binding.as_str().to_string(),
                detail: format!("rdf:List cell <{cursor}> missing rdf:rest"),
            }
        })?;
        if rest.as_str() == RDF_NIL {
            break;
        }
        cursor = rest;
    }
    Ok(out)
}

fn load_step(store: &Store, step_iri: &NamedNode) -> Result<EscalationStep, RoleBindingReadError> {
    let quads = collect_subject_quads(store, step_iri)?;
    let step_capability = take_one_iri(
        step_iri,
        &quads,
        step_iri,
        IRI_DEC_STEP_CAPABILITY,
        "dec:step_capability",
    )?;
    let triggers = load_triggers(store, &quads)?;
    Ok(EscalationStep {
        step_capability,
        triggers,
    })
}

fn load_triggers(
    store: &Store,
    step_quads: &[Quad],
) -> Result<Vec<TriggerSignal>, RoleBindingReadError> {
    let mut trigger_iris: Vec<NamedNode> = Vec::new();
    for q in step_quads {
        if q.predicate.as_str() != IRI_DEC_TRIGGERS {
            continue;
        }
        if let Term::NamedNode(n) = &q.object {
            if !trigger_iris.iter().any(|m| m == n) {
                trigger_iris.push(n.clone());
            }
        }
    }
    let mut triggers = Vec::with_capacity(trigger_iris.len());
    for trigger_iri in trigger_iris {
        let t_quads = collect_subject_quads(store, &trigger_iri)?;
        let raw = take_one_str(
            &trigger_iri,
            &t_quads,
            &trigger_iri,
            IRI_DEC_TRIGGER_SIGNAL,
            "dec:trigger_signal",
        )?;
        let signal =
            TriggerSignal::try_from_str(&raw).ok_or_else(|| RoleBindingReadError::Malformed {
                iri: trigger_iri.as_str().to_string(),
                detail: format!("unknown dec:trigger_signal {raw:?}"),
            })?;
        triggers.push(signal);
    }
    Ok(triggers)
}

fn collect_subject_quads(
    store: &Store,
    iri: &NamedNode,
) -> Result<Vec<Quad>, RoleBindingReadError> {
    let mut out = Vec::new();
    let subj = Subject::NamedNode(iri.clone());
    for quad in store.quads_for_pattern(Some(subj.as_ref()), None, None, None) {
        let q = quad.map_err(|e| RoleBindingReadError::Store(e.to_string()))?;
        out.push(q);
    }
    Ok(out)
}

fn first_object_iri(
    store: &Store,
    subject: &NamedNode,
    predicate: &str,
) -> Result<Option<NamedNode>, RoleBindingReadError> {
    let subj = Subject::NamedNode(subject.clone());
    let pred = NamedNode::new_unchecked(predicate);
    for quad in store.quads_for_pattern(Some(subj.as_ref()), Some(pred.as_ref()), None, None) {
        let q = quad.map_err(|e| RoleBindingReadError::Store(e.to_string()))?;
        if let Term::NamedNode(n) = q.object {
            return Ok(Some(n));
        }
    }
    Ok(None)
}

fn require_type(subject: &NamedNode, quads: &[Quad]) -> Result<(), RoleBindingReadError> {
    let has_type = quads.iter().any(|q| {
        q.predicate.as_str() == RDF_TYPE
            && matches!(&q.object, Term::NamedNode(n) if n.as_str() == IRI_DEC_ROLE_BINDING)
            && matches!(&q.subject, Subject::NamedNode(s) if s == subject)
    });
    if has_type {
        Ok(())
    } else {
        Err(RoleBindingReadError::Malformed {
            iri: subject.as_str().to_string(),
            detail: "missing rdf:type dec:RoleBinding".to_string(),
        })
    }
}
