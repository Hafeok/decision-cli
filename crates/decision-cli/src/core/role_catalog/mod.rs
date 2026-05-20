//! Role catalog backed by the orchestration graph (FT-019).
//!
//! Slice 1 shipped a single hardcoded `implementer` role; Phase A adds
//! the verifier and treats the catalog as a queryable graph artifact.
//! Per the slice-level SDP convention in `CLAUDE.md`, this module lives
//! under `core/`: every feature reads the catalog through here, never
//! through a sibling feature.
//!
//! Slice 2 scale is `N=2` roles — implementer (still hardcoded inline,
//! migrated by FT-030) and verifier (graph-resident here). There is no
//! caching; lookups run a small SPARQL query against the orchestration
//! store each time.

use anyhow::{anyhow, Context, Result};
use oxigraph::model::{GraphName, Literal, NamedNode, NamedNodeRef, Quad, Term};
use oxigraph::sparql::QueryResults;
use oxigraph::store::Store;

use crate::core::vocab::orchestration_graph;

/// Raw Turtle bytes for the verifier seed — embedded via `include_str!`
/// per ADR-007. Loaded into the orchestration store at `dec init` time
/// by [`crate::features::init`].
pub const VERIFIER_SEED_TTL: &str = include_str!("seeds/verifier.ttl");

/// Stable IRI minted for the verifier role catalog entry.
pub const VERIFIER_ROLE_IRI: &str = "https://decision-cli.dev/ns/role/verifier";

/// Stable string id (`dec:roleId`) used to dispatch the verifier.
pub const VERIFIER_ROLE_ID: &str = "verifier";

/// `dec:Role` IRI.
pub const ROLE_CLASS_IRI: &str = "https://decision-cli.dev/ns#Role";

/// `dec:roleId` IRI.
pub const ROLE_ID_IRI: &str = "https://decision-cli.dev/ns#roleId";

/// `dec:roleInputType` IRI.
pub const ROLE_INPUT_TYPE_IRI: &str = "https://decision-cli.dev/ns#roleInputType";

/// `dec:roleOutputType` IRI.
pub const ROLE_OUTPUT_TYPE_IRI: &str = "https://decision-cli.dev/ns#roleOutputType";

/// `dec:roleModelBinding` IRI.
pub const ROLE_MODEL_BINDING_IRI: &str = "https://decision-cli.dev/ns#roleModelBinding";

/// `dec:VerificationVerdict` IRI (FT-020 / ADR-018). Captured here so
/// FT-019 can assert the invariant "verifier emits a VerificationVerdict"
/// without depending on the slice-2 feature module.
pub const VERIFICATION_VERDICT_IRI: &str = "https://decision-cli.dev/ns#VerificationVerdict";

const RDF_TYPE: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#type";

/// A role catalog entry as read from the orchestration store.
///
/// Field semantics mirror the `dec:Role` shape (FT-019):
/// - `iri` — the role artifact's stable IRI.
/// - `role_id` — the string id callers dispatch against.
/// - `input_types` — artifact class IRIs the role consumes.
/// - `output_type` — the artifact class IRI the role produces.
/// - `model_binding` — slice-2 hardcoded model id (ADR-020).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Role {
    /// IRI of the `dec:Role` artifact.
    pub iri: String,
    /// `dec:roleId` — string callers dispatch against.
    pub role_id: String,
    /// `dec:roleInputType` values — IRIs of consumed artifact classes.
    pub input_types: Vec<String>,
    /// `dec:roleOutputType` — IRI of the produced artifact class.
    pub output_type: String,
    /// `dec:roleModelBinding` — Phase A inline model id.
    pub model_binding: String,
}

/// Look up a role by its `dec:roleId` against the orchestration store.
///
/// Returns `None` when the store has no matching role artifact (e.g. a
/// pre-FT-019 store that has not been reseeded). Callers in slice 2
/// (notably `features/ft_021_dispatch_group/`) translate `None` into a
/// structured dispatch error.
pub fn lookup(store: &Store, role_id: &str) -> Result<Option<Role>> {
    let q = lookup_query(role_id);
    let solutions = match store.query(q.as_str()).context("role lookup query")? {
        QueryResults::Solutions(s) => s,
        _ => return Ok(None),
    };
    let Some(row) = collect_first_solution(solutions)? else {
        return Ok(None);
    };
    let (iri, output_type, model_binding) = extract_role_columns(&row)?;
    let input_types = collect_input_types(store, &iri)?;
    Ok(Some(Role {
        iri,
        role_id: role_id.to_string(),
        input_types,
        output_type,
        model_binding,
    }))
}

/// Build the role-lookup SPARQL query. Quads may live in the
/// orchestration named graph or in the default graph (depending on how
/// the store was bootstrapped); the UNION matches either.
fn lookup_query(role_id: &str) -> String {
    format!(
        "PREFIX dec: <https://decision-cli.dev/ns#> \
         SELECT ?role ?out ?model WHERE {{ \
           {{ ?role a dec:Role ; \
                 dec:roleId \"{rid}\" ; \
                 dec:roleOutputType ?out ; \
                 dec:roleModelBinding ?model . }} \
           UNION \
           {{ GRAPH ?g {{ ?role a dec:Role ; \
                 dec:roleId \"{rid}\" ; \
                 dec:roleOutputType ?out ; \
                 dec:roleModelBinding ?model . }} }} \
         }} LIMIT 1",
        rid = sparql_escape_string(role_id),
    )
}

/// Read the full role catalog as a `Vec<Role>`. Order is `dec:roleId`
/// ascending. Designed for slice-2 introspection (`dec role list`,
/// later); slice-2 has at most two entries.
pub fn list_all(store: &Store) -> Result<Vec<Role>> {
    let mut out: Vec<Role> = Vec::new();
    let QueryResults::Solutions(sols) = store.query(LIST_ALL_QUERY).context("role list query")?
    else {
        return Ok(out);
    };
    for sol in sols {
        let sol = sol.context("role list row")?;
        out.push(role_from_row(store, &sol)?);
    }
    Ok(out)
}

const LIST_ALL_QUERY: &str = "PREFIX dec: <https://decision-cli.dev/ns#> \
         SELECT ?role ?rid ?out ?model WHERE { \
           { ?role a dec:Role ; \
                 dec:roleId ?rid ; \
                 dec:roleOutputType ?out ; \
                 dec:roleModelBinding ?model . } \
           UNION \
           { GRAPH ?g { ?role a dec:Role ; \
                 dec:roleId ?rid ; \
                 dec:roleOutputType ?out ; \
                 dec:roleModelBinding ?model . } } \
         } ORDER BY ?rid";

fn role_from_row(store: &Store, sol: &oxigraph::sparql::QuerySolution) -> Result<Role> {
    let iri = require_named_node(sol, "role")?;
    let role_id = require_literal(sol, "rid")?;
    let output_type = require_named_node(sol, "out")?;
    let model_binding = require_literal(sol, "model")?;
    let input_types = collect_input_types(store, &iri)?;
    Ok(Role {
        iri,
        role_id,
        input_types,
        output_type,
        model_binding,
    })
}

/// Build the quad set that seeds the verifier role into the orchestration
/// store. Used by the init pipeline; isolated here so the catalog module
/// owns its own write surface.
#[must_use]
pub fn verifier_seed_quads() -> Vec<Quad> {
    let g: GraphName = orchestration_graph().into_owned().into();
    let role = NamedNode::new_unchecked(VERIFIER_ROLE_IRI);
    let mut quads = role_typing_quads(&role, &g);
    quads.extend(role_input_quads(&role, &g));
    quads.extend(role_output_and_model_quads(&role, &g));
    quads
}

fn role_typing_quads(role: &NamedNode, g: &GraphName) -> Vec<Quad> {
    let rdf_type = NamedNodeRef::new_unchecked(RDF_TYPE).into_owned();
    let role_class = NamedNodeRef::new_unchecked(ROLE_CLASS_IRI).into_owned();
    let role_id = NamedNodeRef::new_unchecked(ROLE_ID_IRI).into_owned();
    vec![
        Quad::new(role.clone(), rdf_type, role_class, g.clone()),
        Quad::new(
            role.clone(),
            role_id,
            Literal::new_simple_literal(VERIFIER_ROLE_ID),
            g.clone(),
        ),
    ]
}

fn role_input_quads(role: &NamedNode, g: &GraphName) -> Vec<Quad> {
    let role_in = NamedNodeRef::new_unchecked(ROLE_INPUT_TYPE_IRI).into_owned();
    let code_change = NamedNode::new_unchecked("https://decision-cli.dev/ns#CodeChange");
    let feature_spec = NamedNode::new_unchecked("https://decision-cli.dev/ns#FeatureSpec");
    let bundle_hash = NamedNode::new_unchecked("https://decision-cli.dev/ns#BundleHash");
    vec![
        Quad::new(role.clone(), role_in.clone(), code_change, g.clone()),
        Quad::new(role.clone(), role_in.clone(), feature_spec, g.clone()),
        Quad::new(role.clone(), role_in, bundle_hash, g.clone()),
    ]
}

fn role_output_and_model_quads(role: &NamedNode, g: &GraphName) -> Vec<Quad> {
    let role_out = NamedNodeRef::new_unchecked(ROLE_OUTPUT_TYPE_IRI).into_owned();
    let role_model = NamedNodeRef::new_unchecked(ROLE_MODEL_BINDING_IRI).into_owned();
    let verdict = NamedNode::new_unchecked(VERIFICATION_VERDICT_IRI);
    vec![
        Quad::new(role.clone(), role_out, verdict, g.clone()),
        Quad::new(
            role.clone(),
            role_model,
            Literal::new_simple_literal("claude-sonnet-4-5"),
            g.clone(),
        ),
    ]
}

fn collect_first_solution(
    mut sols: oxigraph::sparql::QuerySolutionIter,
) -> Result<Option<oxigraph::sparql::QuerySolution>> {
    match sols.next() {
        Some(Ok(s)) => Ok(Some(s)),
        Some(Err(e)) => Err(anyhow!("role lookup row: {e}")),
        None => Ok(None),
    }
}

fn extract_role_columns(row: &oxigraph::sparql::QuerySolution) -> Result<(String, String, String)> {
    let iri = require_named_node(row, "role")?;
    let output_type = require_named_node(row, "out")?;
    let model_binding = require_literal(row, "model")?;
    Ok((iri, output_type, model_binding))
}

fn collect_input_types(store: &Store, role_iri: &str) -> Result<Vec<String>> {
    let q = format!(
        "PREFIX dec: <https://decision-cli.dev/ns#> \
         SELECT ?t WHERE {{ \
           {{ <{role}> dec:roleInputType ?t . }} \
           UNION \
           {{ GRAPH ?g {{ <{role}> dec:roleInputType ?t . }} }} \
         }} ORDER BY ?t",
        role = role_iri,
    );
    let mut out: Vec<String> = Vec::new();
    let QueryResults::Solutions(sols) = store.query(q.as_str()).context("input-type query")?
    else {
        return Ok(out);
    };
    for sol in sols {
        let sol = sol.context("input-type row")?;
        if let Some(Term::NamedNode(n)) = sol.get("t") {
            out.push(n.as_str().to_string());
        }
    }
    Ok(out)
}

fn require_named_node(row: &oxigraph::sparql::QuerySolution, name: &str) -> Result<String> {
    match row.get(name) {
        Some(Term::NamedNode(n)) => Ok(n.as_str().to_string()),
        Some(other) => Err(anyhow!("expected IRI for ?{name}, got {other}")),
        None => Err(anyhow!("missing binding for ?{name}")),
    }
}

fn require_literal(row: &oxigraph::sparql::QuerySolution, name: &str) -> Result<String> {
    match row.get(name) {
        Some(Term::Literal(lit)) => Ok(lit.value().to_string()),
        Some(other) => Err(anyhow!("expected literal for ?{name}, got {other}")),
        None => Err(anyhow!("missing binding for ?{name}")),
    }
}

fn sparql_escape_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seeded_store() -> Store {
        let store = Store::new().expect("in-memory store");
        store
            .transaction(|mut tx| {
                for q in verifier_seed_quads() {
                    tx.insert(q.as_ref())?;
                }
                Ok::<_, oxigraph::store::StorageError>(())
            })
            .expect("seed verifier quads");
        store
    }

    #[test]
    fn lookup_verifier_after_seed_returns_some() {
        let store = seeded_store();
        let role = lookup(&store, VERIFIER_ROLE_ID)
            .expect("lookup ok")
            .expect("verifier present after seed");
        assert_eq!(role.role_id, VERIFIER_ROLE_ID);
        assert_eq!(role.iri, VERIFIER_ROLE_IRI);
        assert_eq!(role.output_type, VERIFICATION_VERDICT_IRI);
        assert_eq!(role.model_binding, "claude-sonnet-4-5");
        assert_eq!(role.input_types.len(), 3);
        assert!(role
            .input_types
            .iter()
            .any(|t| t == "https://decision-cli.dev/ns#CodeChange"));
        assert!(role
            .input_types
            .iter()
            .any(|t| t == "https://decision-cli.dev/ns#FeatureSpec"));
        assert!(role
            .input_types
            .iter()
            .any(|t| t == "https://decision-cli.dev/ns#BundleHash"));
    }

    #[test]
    fn lookup_returns_none_on_empty_store() {
        let store = Store::new().expect("in-memory store");
        let role = lookup(&store, VERIFIER_ROLE_ID).expect("lookup ok");
        assert!(role.is_none());
    }

    #[test]
    fn lookup_returns_none_for_unknown_role() {
        let store = seeded_store();
        let role = lookup(&store, "no-such-role").expect("lookup ok");
        assert!(role.is_none());
    }

    #[test]
    fn list_all_includes_verifier() {
        let store = seeded_store();
        let roles = list_all(&store).expect("list ok");
        assert_eq!(roles.len(), 1);
        assert_eq!(roles[0].role_id, VERIFIER_ROLE_ID);
    }

    #[test]
    fn seed_ttl_mentions_required_predicates() {
        // FT-019 invariant: the embedded seed actually declares the
        // four required Role predicates (matches RoleShape in shapes.ttl).
        for pred in [
            "dec:roleId",
            "dec:roleInputType",
            "dec:roleOutputType",
            "dec:roleModelBinding",
        ] {
            assert!(
                VERIFIER_SEED_TTL.contains(pred),
                "verifier seed missing {pred}"
            );
        }
    }
}
