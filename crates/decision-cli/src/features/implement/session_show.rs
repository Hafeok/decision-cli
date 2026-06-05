//! `dec session show <iri>` — render a Session by IRI.
//!
//! When the session belongs to a `dec:DispatchGroup` (FT-021 / ADR-017),
//! a `Paired:` block is appended after the slice-1 fields, naming the
//! group's status, the paired session (action or interpretation), and
//! — when available — the verifier's `VerificationVerdict` with its
//! rationale, violates list, and amendment guidance (FT-025 / ADR-018).

mod cluster;
mod paired;

use std::fs;
use std::path::Path;

use anyhow::{anyhow, Context, Result};
use oxigraph::io::RdfFormat;
use oxigraph::store::Store;

use super::vocab::DEC_NS;

use paired::{term_iri, term_literal};

/// FT-161: IRI prefix for per-cell cluster sessions persisted by
/// [`crate::core::graph::cluster_session::persist_cluster_run`].
const CLUSTER_CELL_PREFIX: &str = "urn:dec:cluster-session:";
/// FT-161: IRI prefix for the parent cluster activity persisted by
/// FT-146's `persist_cluster_run`.
const CLUSTER_DISPATCH_PREFIX: &str = "urn:dec:cluster-dispatch:";

/// `dec session show <iri>` payload — load Session triples and render.
///
/// FT-161 routes cluster-shape IRIs to dedicated renderers before
/// touching the slice-1 implementer-shape SPARQL.
pub fn session_show(workdir: &Path, session_iri: &str) -> Result<String> {
    let store = load_orchestration_store(workdir)?;
    if session_iri.starts_with(CLUSTER_CELL_PREFIX) {
        return cluster::render_cluster_cell(&store, session_iri);
    }
    if session_iri.starts_with(CLUSTER_DISPATCH_PREFIX) {
        return cluster::render_cluster_dispatch(&store, session_iri);
    }
    let q = build_session_show_query(session_iri);
    let sol = fetch_session_solution(&store, q.as_str(), session_iri)?;
    let mut report = render_session_solution(&sol, session_iri);
    if let Some(paired) = paired::lookup_paired_block(&store, session_iri)? {
        report.push_str(&paired);
    }
    Ok(report)
}

fn load_orchestration_store(workdir: &Path) -> Result<Store> {
    let dump_path = workdir.join(".dec").join("store").join("orchestration.nq");
    let bytes = fs::read(&dump_path).with_context(|| format!("reading {}", dump_path.display()))?;
    let store = Store::new().context("opening session-show store")?;
    store
        .load_from_reader(RdfFormat::NQuads, bytes.as_slice())
        .with_context(|| format!("loading {}", dump_path.display()))?;
    Ok(store)
}

fn build_session_show_query(session_iri: &str) -> String {
    format!(
        "PREFIX dec: <{DEC_NS}>
PREFIX prov: <http://www.w3.org/ns/prov#>
SELECT ?bundleHash ?modelVersion ?feature ?status ?output ?stream WHERE {{
  GRAPH ?g {{
    <{session_iri}> prov:used ?bundle ;
                    prov:used ?model ;
                    dec:featureId ?feature ;
                    dec:inStream ?stream .
    ?bundle dec:contentHash ?bundleHash .
    ?model dec:modelVersion ?modelVersion .
    OPTIONAL {{ <{session_iri}> dec:status ?status }}
    OPTIONAL {{ <{session_iri}> dec:outputRef ?output }}
  }}
}} LIMIT 1",
    )
}

fn fetch_session_solution(
    store: &Store,
    query: &str,
    session_iri: &str,
) -> Result<oxigraph::sparql::QuerySolution> {
    let results = store.query(query).context("session-show SPARQL")?;
    let oxigraph::sparql::QueryResults::Solutions(mut sols) = results else {
        return Err(anyhow!("session-show: unexpected SPARQL result shape"));
    };
    let Some(sol_res) = sols.next() else {
        return Err(anyhow!("session-show: no Session with IRI <{session_iri}>"));
    };
    sol_res.context("session-show SPARQL solution")
}

fn render_session_solution(sol: &oxigraph::sparql::QuerySolution, session_iri: &str) -> String {
    let bh = sol
        .get("bundleHash")
        .map(term_literal)
        .unwrap_or_else(|| "(unknown)".into());
    let mv = sol
        .get("modelVersion")
        .map(term_literal)
        .unwrap_or_else(|| "(unknown)".into());
    let ft = sol
        .get("feature")
        .map(term_literal)
        .unwrap_or_else(|| "(unknown)".into());
    let st = sol
        .get("status")
        .map(term_literal)
        .unwrap_or_else(|| "(pending)".into());
    let ou = sol
        .get("output")
        .map(term_iri)
        .unwrap_or_else(|| "(none)".into());
    let sr = sol
        .get("stream")
        .map(term_iri)
        .unwrap_or_else(|| "(unknown)".into());
    format!(
        "Session: {session_iri}\n  Feature:        {ft}\n  Bundle hash:    {bh}\n  Model version:  {mv}\n  Stream:         {sr}\n  Status:         {st}\n  Output:         {ou}\n",
    )
}
