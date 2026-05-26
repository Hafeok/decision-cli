//! Full-chain QueryTemplate accessors (FT-075 / ADR-043).
//!
//! Fetches `dec:QueryTemplate` artifacts from the orchestration store and
//! executes their `dec:querySpec` against the store with caller-provided
//! variable bindings. The slice-1 canonical instances —
//! `qt:full-chain-backward-v1` and `qt:full-chain-forward-v1` — are
//! shipped as bootstrap TTL fixtures embedded via `include_str!`.

use oxigraph::io::RdfFormat;
use oxigraph::model::{GraphName, Quad};
use oxigraph::sparql::QueryResults;
use oxigraph::store::Store;
use thiserror::Error;

/// IRI of the `dec:QueryTemplate` rdfs:Class.
pub const QUERY_TEMPLATE_CLASS_IRI: &str = "https://decision-cli.dev/ns#QueryTemplate";

/// Shorthand identifier (used by the CLI `dec query template show <id>`)
/// for the slice-1 backward traversal.
pub const FULL_CHAIN_BACKWARD_ID: &str = "qt:full-chain-backward-v1";

/// Stable IRI of the slice-1 backward traversal instance.
pub const FULL_CHAIN_BACKWARD_IRI: &str =
    "https://decision-cli.dev/ns/qt/full-chain-backward-v1";

/// Bootstrap Turtle bytes for the backward template, seeded at `dec init`.
pub const FULL_CHAIN_BACKWARD_TTL: &str =
    include_str!("bootstrap/qt-full-chain-backward-v1.ttl");

/// Shorthand identifier for the slice-1 forward traversal.
pub const FULL_CHAIN_FORWARD_ID: &str = "qt:full-chain-forward-v1";

/// Stable IRI of the slice-1 forward traversal instance.
pub const FULL_CHAIN_FORWARD_IRI: &str =
    "https://decision-cli.dev/ns/qt/full-chain-forward-v1";

/// Bootstrap Turtle bytes for the forward template, seeded at `dec init`.
pub const FULL_CHAIN_FORWARD_TTL: &str =
    include_str!("bootstrap/qt-full-chain-forward-v1.ttl");

const DEC_QUERY_SPEC: &str = "https://decision-cli.dev/ns#querySpec";
const DEC_QUERY_LANGUAGE: &str = "https://decision-cli.dev/ns#queryLanguage";
const DEC_VERSION: &str = "https://decision-cli.dev/ns#version";

/// A `dec:QueryTemplate` artifact materialised into a Rust struct.
#[derive(Debug, Clone)]
pub struct QueryTemplate {
    /// Stable IRI of the template instance.
    pub iri: String,
    /// Shorthand identifier matching the IRI's local-name (e.g.
    /// `qt:full-chain-backward-v1`).
    pub id: String,
    /// Verbatim SPARQL source — the `dec:querySpec` literal.
    pub spec: String,
    /// Query language identifier (currently always `"SPARQL-1.1"`).
    pub language: String,
    /// Semver-style version (e.g. `"1.0.0"`).
    pub version: String,
}

/// Structured failures from the QueryTemplate accessors.
#[allow(missing_docs)]
#[derive(Debug, Error)]
pub enum QueryTemplateError {
    #[error("QueryTemplate `{id}` not found in the orchestration store")]
    TemplateNotFound { id: String },

    #[error(
        "QueryTemplate `{id}` is malformed: missing required field {field}"
    )]
    MalformedTemplate { id: String, field: &'static str },

    #[error("SPARQL execution failed for QueryTemplate `{id}`: {detail}")]
    QueryExecution { id: String, detail: String },

    #[error("orchestration store error: {0}")]
    Store(String),
}

/// Build the quads that seed both slice-1 QueryTemplate instances into
/// the orchestration store. Used by `dec init`'s pipeline alongside the
/// role catalog and subscription seeds.
#[must_use]
pub fn bootstrap_query_template_quads() -> Vec<Quad> {
    let mut quads = parse_bootstrap_ttl(FULL_CHAIN_BACKWARD_TTL);
    quads.extend(parse_bootstrap_ttl(FULL_CHAIN_FORWARD_TTL));
    quads
}

fn parse_bootstrap_ttl(ttl: &str) -> Vec<Quad> {
    let scratch = Store::new().expect("in-memory oxigraph store opens for parsing bootstrap TTL");
    scratch
        .load_from_reader(RdfFormat::Turtle, ttl.as_bytes())
        .expect("bootstrap TTL parses (compile-time asset)");
    // Bootstrap QueryTemplate instances live in the SPARQL default graph
    // — consistent with the bulk of init's session/value-stream/value-
    // action data — so the verbatim template specs work without `GRAPH`
    // wrapping. The fetch/list helpers below UNION default + named so a
    // future migration that re-graphs the catalog still resolves.
    let mut out = Vec::new();
    for quad_res in scratch.iter() {
        let q = quad_res.expect("scratch store iteration");
        out.push(Quad::new(
            q.subject,
            q.predicate,
            q.object,
            GraphName::DefaultGraph,
        ));
    }
    out
}

/// List every `dec:QueryTemplate` instance in `store`, sorted by IRI.
pub fn list_query_templates(store: &Store) -> Result<Vec<QueryTemplate>, QueryTemplateError> {
    let q = build_list_query();
    let results = store
        .query(q.as_str())
        .map_err(|e| QueryTemplateError::Store(e.to_string()))?;
    let QueryResults::Solutions(sols) = results else {
        return Ok(Vec::new());
    };
    let mut out = Vec::new();
    for sol in sols {
        let sol = sol.map_err(|e| QueryTemplateError::Store(e.to_string()))?;
        if let Some(template) = parse_list_solution(&sol) {
            out.push(template);
        }
    }
    Ok(out)
}

fn build_list_query() -> String {
    format!(
        "PREFIX dec: <https://decision-cli.dev/ns#>\n\
         PREFIX rdf: <http://www.w3.org/1999/02/22-rdf-syntax-ns#>\n\
         SELECT ?iri ?spec ?lang ?ver WHERE {{ \
             {{ ?iri rdf:type <{cls}> ; \
                    <{spec_pred}> ?spec ; \
                    <{lang_pred}> ?lang ; \
                    <{ver_pred}> ?ver . }} \
             UNION \
             {{ GRAPH ?g {{ ?iri rdf:type <{cls}> ; \
                    <{spec_pred}> ?spec ; \
                    <{lang_pred}> ?lang ; \
                    <{ver_pred}> ?ver . }} }} \
         }} ORDER BY ?iri",
        cls = QUERY_TEMPLATE_CLASS_IRI,
        spec_pred = DEC_QUERY_SPEC,
        lang_pred = DEC_QUERY_LANGUAGE,
        ver_pred = DEC_VERSION,
    )
}

fn parse_list_solution(sol: &oxigraph::sparql::QuerySolution) -> Option<QueryTemplate> {
    let iri_term = sol.get("iri")?;
    let oxigraph::model::Term::NamedNode(iri_node) = iri_term else {
        return None;
    };
    let iri = iri_node.as_str().to_string();
    Some(QueryTemplate {
        id: short_id_from_iri(&iri),
        iri,
        spec: literal_value(sol.get("spec")).unwrap_or_default(),
        language: literal_value(sol.get("lang")).unwrap_or_default(),
        version: literal_value(sol.get("ver")).unwrap_or_default(),
    })
}

/// Fetch a single `dec:QueryTemplate` by id (shorthand `qt:foo` or full
/// IRI) from `store`. Returns [`QueryTemplateError::TemplateNotFound`]
/// when no matching instance exists.
pub fn fetch_query_template(
    store: &Store,
    id: &str,
) -> Result<QueryTemplate, QueryTemplateError> {
    let iri = resolve_id_to_iri(id);
    let q = build_fetch_query(&iri);
    let results = store
        .query(q.as_str())
        .map_err(|e| QueryTemplateError::Store(e.to_string()))?;
    let QueryResults::Solutions(mut sols) = results else {
        return Err(QueryTemplateError::TemplateNotFound { id: id.to_string() });
    };
    let Some(sol) = sols.next() else {
        return Err(QueryTemplateError::TemplateNotFound { id: id.to_string() });
    };
    let sol = sol.map_err(|e| QueryTemplateError::Store(e.to_string()))?;
    parse_fetch_solution(&sol, id, iri)
}

fn build_fetch_query(iri: &str) -> String {
    format!(
        "PREFIX dec: <https://decision-cli.dev/ns#>\n\
         PREFIX rdf: <http://www.w3.org/1999/02/22-rdf-syntax-ns#>\n\
         SELECT ?spec ?lang ?ver WHERE {{ \
             {{ <{iri}> rdf:type <{cls}> ; \
                       <{spec_pred}> ?spec ; \
                       <{lang_pred}> ?lang ; \
                       <{ver_pred}> ?ver . }} \
             UNION \
             {{ GRAPH ?g {{ <{iri}> rdf:type <{cls}> ; \
                       <{spec_pred}> ?spec ; \
                       <{lang_pred}> ?lang ; \
                       <{ver_pred}> ?ver . }} }} \
         }} LIMIT 1",
        iri = iri,
        cls = QUERY_TEMPLATE_CLASS_IRI,
        spec_pred = DEC_QUERY_SPEC,
        lang_pred = DEC_QUERY_LANGUAGE,
        ver_pred = DEC_VERSION,
    )
}

fn parse_fetch_solution(
    sol: &oxigraph::sparql::QuerySolution,
    id: &str,
    iri: String,
) -> Result<QueryTemplate, QueryTemplateError> {
    let spec = literal_value(sol.get("spec")).ok_or(QueryTemplateError::MalformedTemplate {
        id: id.to_string(),
        field: "dec:querySpec",
    })?;
    let language = literal_value(sol.get("lang")).ok_or(QueryTemplateError::MalformedTemplate {
        id: id.to_string(),
        field: "dec:queryLanguage",
    })?;
    let version = literal_value(sol.get("ver")).ok_or(QueryTemplateError::MalformedTemplate {
        id: id.to_string(),
        field: "dec:version",
    })?;
    Ok(QueryTemplate {
        id: short_id_from_iri(&iri),
        iri,
        spec,
        language,
        version,
    })
}

/// Execute `template` against `store` with caller-provided bindings
/// substituted into the query before parsing. Bindings are inserted as a
/// `VALUES` block at the head of the WHERE clause so the SPARQL engine
/// performs the actual binding — no string templating into the spec
/// itself. Each binding is `(variable_name, full_iri)`; the variable is
/// bound to the named node.
pub fn execute_template(
    store: &Store,
    template: &QueryTemplate,
    bindings: &[(&str, &str)],
) -> Result<QueryResults, QueryTemplateError> {
    let query = compose_query_with_bindings(&template.spec, bindings);
    store
        .query(query.as_str())
        .map_err(|e| QueryTemplateError::QueryExecution {
            id: template.id.clone(),
            detail: e.to_string(),
        })
}

fn compose_query_with_bindings(spec: &str, bindings: &[(&str, &str)]) -> String {
    if bindings.is_empty() {
        return spec.to_string();
    }
    // Build a `VALUES (?v1 ?v2) { (<iri1> <iri2>) }` clause and inject it
    // immediately after the WHERE {. Oxigraph's SPARQL parser accepts a
    // values block in any group pattern; placing it at the WHERE head
    // pins the focal variable(s) without rewriting the original query.
    let mut vars = String::new();
    let mut row = String::new();
    for (name, iri) in bindings {
        if !vars.is_empty() {
            vars.push(' ');
        }
        vars.push('?');
        vars.push_str(name);
        if !row.is_empty() {
            row.push(' ');
        }
        row.push('<');
        row.push_str(iri);
        row.push('>');
    }
    let values_clause = format!("    VALUES ({vars}) {{ ({row}) }}\n");
    // Find first WHERE { and inject. Case-insensitive on WHERE.
    if let Some(idx) = find_where_open_brace(spec) {
        let mut out = String::with_capacity(spec.len() + values_clause.len());
        out.push_str(&spec[..=idx]);
        out.push('\n');
        out.push_str(&values_clause);
        out.push_str(&spec[idx + 1..]);
        out
    } else {
        // Fall back to passing the spec through unchanged; the engine
        // will return whatever the unbound query returns.
        spec.to_string()
    }
}

fn find_where_open_brace(spec: &str) -> Option<usize> {
    // Find the `{` that opens the top-level WHERE clause. Scan forward
    // looking for the literal `WHERE` (case-insensitive) followed by an
    // optional whitespace run and then `{`.
    let lower = spec.to_ascii_lowercase();
    let mut search_from = 0usize;
    while let Some(pos) = lower[search_from..].find("where") {
        let abs = search_from + pos;
        let after = abs + "where".len();
        let rest = &spec[after..];
        for (i, ch) in rest.char_indices() {
            if ch.is_whitespace() {
                continue;
            }
            if ch == '{' {
                return Some(after + i);
            }
            break;
        }
        search_from = after;
    }
    None
}

fn literal_value(term: Option<&oxigraph::model::Term>) -> Option<String> {
    match term? {
        oxigraph::model::Term::Literal(lit) => Some(lit.value().to_string()),
        oxigraph::model::Term::NamedNode(n) => Some(n.as_str().to_string()),
        _ => None,
    }
}

fn resolve_id_to_iri(id: &str) -> String {
    if id.starts_with("https://") || id.starts_with("http://") {
        return id.to_string();
    }
    if let Some(local) = id.strip_prefix("qt:") {
        return format!("https://decision-cli.dev/ns/qt/{local}");
    }
    // Pre-canonicalised slice-1 ids fall back through a lookup table so
    // operators can type bare ids like `full-chain-backward-v1`.
    match id {
        FULL_CHAIN_BACKWARD_ID => FULL_CHAIN_BACKWARD_IRI.to_string(),
        FULL_CHAIN_FORWARD_ID => FULL_CHAIN_FORWARD_IRI.to_string(),
        _ => id.to_string(),
    }
}

fn short_id_from_iri(iri: &str) -> String {
    // Convert `https://decision-cli.dev/ns/qt/foo` → `qt:foo`.
    if let Some(local) = iri.strip_prefix("https://decision-cli.dev/ns/qt/") {
        return format!("qt:{local}");
    }
    iri.to_string()
}

// Unit tests for this module live in the integration test
// `crates/decision-cli/tests/ft_075_full_chain_query.rs` so the public
// surface is exercised through the same boundary external callers use
// (and so this source file stays under ADR-013's per-file size cap).
