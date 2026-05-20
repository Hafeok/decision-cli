//! `dec session show <iri>` — render a Session by IRI.

use std::fs;
use std::path::Path;

use anyhow::{anyhow, Context, Result};
use oxigraph::io::RdfFormat;
use oxigraph::store::Store;

use super::vocab::DEC_NS;

/// `dec session show <iri>` payload — load Session triples and render.
pub fn session_show(workdir: &Path, session_iri: &str) -> Result<String> {
    let store = load_orchestration_store(workdir)?;
    let q = build_session_show_query(session_iri);
    let sol = fetch_session_solution(&store, q.as_str(), session_iri)?;
    Ok(render_session_solution(&sol, session_iri))
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

fn term_literal(t: &oxigraph::model::Term) -> String {
    match t {
        oxigraph::model::Term::Literal(lit) => lit.value().to_string(),
        oxigraph::model::Term::NamedNode(n) => n.as_str().to_string(),
        other => other.to_string(),
    }
}

fn term_iri(t: &oxigraph::model::Term) -> String {
    match t {
        oxigraph::model::Term::NamedNode(n) => n.as_str().to_string(),
        other => other.to_string(),
    }
}
