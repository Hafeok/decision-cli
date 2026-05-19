//! `dec session show <iri>` — render a Session by IRI.

use std::fs;
use std::path::Path;

use anyhow::{anyhow, Context, Result};
use oxigraph::io::RdfFormat;
use oxigraph::store::Store;

use super::vocab::DEC_NS;

/// `dec session show <iri>` payload — load Session triples and render.
pub fn session_show(workdir: &Path, session_iri: &str) -> Result<String> {
    let dump_path = workdir.join(".dec").join("store").join("orchestration.nq");
    let bytes =
        fs::read(&dump_path).with_context(|| format!("reading {}", dump_path.display()))?;
    let store = Store::new().context("opening session-show store")?;
    store
        .load_from_reader(RdfFormat::NQuads, bytes.as_slice())
        .with_context(|| format!("loading {}", dump_path.display()))?;

    let q = format!(
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
    );

    let results = store.query(q.as_str()).context("session-show SPARQL")?;
    let oxigraph::sparql::QueryResults::Solutions(mut sols) = results else {
        return Err(anyhow!("session-show: unexpected SPARQL result shape"));
    };
    let Some(sol_res) = sols.next() else {
        return Err(anyhow!("session-show: no Session with IRI <{session_iri}>"));
    };
    let sol = sol_res.context("session-show SPARQL solution")?;
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

    Ok(format!(
        "Session: {session_iri}\n  Feature:        {ft}\n  Bundle hash:    {bh}\n  Model version:  {mv}\n  Stream:         {sr}\n  Status:         {st}\n  Output:         {ou}\n",
    ))
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
