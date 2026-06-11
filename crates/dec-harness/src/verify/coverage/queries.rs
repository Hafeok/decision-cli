//! SPARQL query strings for the coverage primitive (FT-045 / ADR-030).
//!
//! [`COVERAGE`] is the load-bearing query: it traverses each
//! `VerificationGraph`'s `dec:steps` rdf:List and binds every
//! `(?graph, ?step, ?tc)` triple where the step declares
//! `dec:providesEvidenceFor ?tc`. The shape mirrors the snippet in
//! [FT-045](FT-045) §Behaviour with one widening: it also binds
//! `?graph` so the same query answers per-graph and roll-up queries
//! with a single store round-trip. The query is exported as a `pub`
//! constant per the spec so downstream code (and `dec _sparql`-style
//! debugging) can reuse the canonical form.

/// SPARQL `SELECT` query returning every `(graph, step, tc)` row where
/// a step declares `dec:providesEvidenceFor tc`. Bind `?graph` /
/// `?tc` in the caller (or filter the result set in Rust).
pub const COVERAGE: &str = "PREFIX dec: <https://decision-cli.dev/ns#>\n\
PREFIX rdf: <http://www.w3.org/1999/02/22-rdf-syntax-ns#>\n\
SELECT ?graph ?step ?tc WHERE {\n\
  GRAPH <https://decision-cli.dev/ns/graph/verify-graph> {\n\
    ?graph a dec:VerificationGraph ;\n\
           dec:steps/rdf:rest*/rdf:first ?step .\n\
    ?step dec:providesEvidenceFor ?tc .\n\
  }\n\
}\n";

/// ASK query: does `<{iri}>` exist as a `dec:VerificationGraph` in the
/// verify-graph named graph?
pub(super) fn graph_exists_query(iri: &str) -> String {
    format!(
        "PREFIX dec: <https://decision-cli.dev/ns#>\n\
         ASK WHERE {{\n  GRAPH <https://decision-cli.dev/ns/graph/verify-graph> {{\n\
    <{iri}> a dec:VerificationGraph .\n  }}\n}}\n"
    )
}

/// SELECT query: every `dec:VerificationGraph` whose `dec:verifies`
/// matches one of `targets`. Used when the caller passes
/// `candidates = None` to discover the relevant set.
pub(super) fn relevant_graphs_query(targets: &[String]) -> String {
    let filter = targets
        .iter()
        .map(|t| format!("<{t}>"))
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "PREFIX dec: <https://decision-cli.dev/ns#>\n\
         SELECT DISTINCT ?graph WHERE {{\n  GRAPH <https://decision-cli.dev/ns/graph/verify-graph> {{\n\
    ?graph a dec:VerificationGraph ;\n           dec:verifies ?target .\n\
    FILTER(?target IN ({filter}))\n  }}\n}}\n ORDER BY ?graph\n"
    )
}
