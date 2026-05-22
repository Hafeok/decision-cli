//! `dec:Role` artifact reads — lookup/list helpers backed by SPARQL.
//!
//! Slice 2 scale is `N=2` roles. Lookups run a small SPARQL query each
//! time; FT-030 adds the `dec:authority` resolution per ADR-027.

use anyhow::{anyhow, Context, Result};
use oxigraph::sparql::QueryResults;
use oxigraph::store::Store;

use super::authority::{Authority, AUTHORITY_PRED_IRI};

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

/// `dec:VerificationVerdict` IRI (FT-020 / ADR-018).
pub const VERIFICATION_VERDICT_IRI: &str = "https://decision-cli.dev/ns#VerificationVerdict";

/// A role catalog entry as read from the orchestration store.
///
/// FT-030 / ADR-027: every role carries exactly one `authority`
/// declaration. The field is `Option<Authority>` to allow degraded
/// reads on legacy stores that haven't been re-seeded; SHACL refuses
/// new writes that omit `dec:authority`.
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
    /// `dec:authority` — ADR-027 declaration. `None` for legacy stores.
    pub authority: Option<Authority>,
}

/// Look up a role by its `dec:roleId` against the orchestration store.
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
    let authority = load_authority_for_role(store, &iri)?;
    Ok(Some(Role {
        iri,
        role_id: role_id.to_string(),
        input_types,
        output_type,
        model_binding,
        authority,
    }))
}

/// Read the full role catalog as a `Vec<Role>`. Order is `dec:roleId`
/// ascending.
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
    let authority = load_authority_for_role(store, &iri)?;
    Ok(Role {
        iri,
        role_id,
        input_types,
        output_type,
        model_binding,
        authority,
    })
}

fn load_authority_for_role(store: &Store, role_iri: &str) -> Result<Option<Authority>> {
    let q = format!(
        "SELECT ?a WHERE {{ \
           {{ <{role}> <{pred}> ?a . }} \
           UNION \
           {{ GRAPH ?g {{ <{role}> <{pred}> ?a . }} }} \
         }} LIMIT 1",
        role = role_iri,
        pred = AUTHORITY_PRED_IRI,
    );
    let QueryResults::Solutions(mut sols) =
        store.query(q.as_str()).context("authority link lookup")?
    else {
        return Ok(None);
    };
    let Some(sol) = sols.next() else {
        return Ok(None);
    };
    let sol = sol.context("authority link row")?;
    let Some(oxigraph::model::Term::NamedNode(node)) = sol.get("a") else {
        return Ok(None);
    };
    Authority::load(store, node.as_str())
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
    let QueryResults::Solutions(sols) = store.query(q.as_str()).context("input-type query")? else {
        return Ok(out);
    };
    for sol in sols {
        let sol = sol.context("input-type row")?;
        if let Some(oxigraph::model::Term::NamedNode(n)) = sol.get("t") {
            out.push(n.as_str().to_string());
        }
    }
    Ok(out)
}

fn require_named_node(row: &oxigraph::sparql::QuerySolution, name: &str) -> Result<String> {
    match row.get(name) {
        Some(oxigraph::model::Term::NamedNode(n)) => Ok(n.as_str().to_string()),
        Some(other) => Err(anyhow!("expected IRI for ?{name}, got {other}")),
        None => Err(anyhow!("missing binding for ?{name}")),
    }
}

fn require_literal(row: &oxigraph::sparql::QuerySolution, name: &str) -> Result<String> {
    match row.get(name) {
        Some(oxigraph::model::Term::Literal(lit)) => Ok(lit.value().to_string()),
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
