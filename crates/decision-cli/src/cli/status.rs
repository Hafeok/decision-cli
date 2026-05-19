//! `dec status` — display the active value stream's identity and bootstrap
//! provenance (FT-012 / TC-006).

use std::path::Path;
use std::process::ExitCode;

use decision_cli::bundled;
use oxigraph::io::RdfFormat;
use oxigraph::sparql::QueryResults;
use oxigraph::store::Store;

pub fn run(workdir: &Path) -> ExitCode {
    let dec_dir = workdir.join(".dec");
    if !dec_dir.exists() {
        eprintln!(
            "dec status: not inside an initialised decision-cli working dir.\n  \
             Run `dec init --template engineering-development` first."
        );
        return ExitCode::from(1);
    }
    let dump_path = dec_dir.join("store").join("orchestration.nq");
    let store = match open_store(&dump_path) {
        Ok(s) => s,
        Err(code) => return code,
    };

    let (stream_iri, stream_name, action_iri) = match read_stream_identity(&store) {
        Ok(t) => t,
        Err(code) => return code,
    };

    let authorized_goals = read_authorized_goals(&store, &stream_iri);
    let (source, hash, ontology_version) = read_bootstrap_provenance(&store);

    print_report(
        &dec_dir,
        &stream_iri,
        &stream_name,
        &action_iri,
        &authorized_goals,
        &source,
        &hash,
        &ontology_version,
    );

    ExitCode::SUCCESS
}

fn open_store(dump_path: &Path) -> Result<Store, ExitCode> {
    let bytes = match std::fs::read(dump_path) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("dec status: failed to read {}: {e}", dump_path.display());
            return Err(ExitCode::from(1));
        }
    };
    let store = match Store::new() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("dec status: failed to open store: {e}");
            return Err(ExitCode::from(1));
        }
    };
    if let Err(e) = store.load_from_reader(RdfFormat::NQuads, bytes.as_slice()) {
        eprintln!("dec status: failed to load store: {e}");
        return Err(ExitCode::from(1));
    }
    Ok(store)
}

fn read_stream_identity(store: &Store) -> Result<(String, String, String), ExitCode> {
    let stream_q = "PREFIX dec: <https://decision-cli.dev/ns#>
SELECT ?stream ?name ?action WHERE {
  ?stream a dec:ValueStream ;
          dec:terminalValueAction ?action .
  OPTIONAL { ?stream dec:name ?name }
} LIMIT 1";
    match store.query(stream_q) {
        Ok(QueryResults::Solutions(mut sols)) => match sols.next() {
            Some(Ok(sol)) => {
                let stream = sol
                    .get("stream")
                    .map(term_iri_string)
                    .unwrap_or_else(|| "(unknown)".into());
                let name = sol
                    .get("name")
                    .map(term_literal_string)
                    .unwrap_or_default();
                let action = sol
                    .get("action")
                    .map(term_iri_string)
                    .unwrap_or_else(|| "(unknown)".into());
                Ok((stream, name, action))
            }
            _ => {
                eprintln!("dec status: no dec:ValueStream artifact in the store");
                Err(ExitCode::from(1))
            }
        },
        Ok(_) => {
            eprintln!("dec status: unexpected SPARQL result shape for stream lookup");
            Err(ExitCode::from(1))
        }
        Err(e) => {
            eprintln!("dec status: SPARQL error: {e}");
            Err(ExitCode::from(1))
        }
    }
}

fn read_authorized_goals(store: &Store, stream_iri: &str) -> Vec<String> {
    let goals_q = format!(
        "PREFIX dec: <https://decision-cli.dev/ns#>
SELECT ?goal WHERE {{ <{stream_iri}> dec:authorizedGoals ?goal }}"
    );
    let mut authorized_goals: Vec<String> = Vec::new();
    if let Ok(QueryResults::Solutions(sols)) = store.query(goals_q.as_str()) {
        for sol in sols.flatten() {
            if let Some(t) = sol.get("goal") {
                let v = term_literal_string(t);
                if !v.is_empty() {
                    authorized_goals.push(v);
                }
            }
        }
    }
    authorized_goals.sort();
    authorized_goals.dedup();
    authorized_goals
}

fn read_bootstrap_provenance(store: &Store) -> (String, String, String) {
    let prov_q = "PREFIX dec:  <https://decision-cli.dev/ns#>
PREFIX prov: <http://www.w3.org/ns/prov#>
SELECT ?source ?hash ?version WHERE {
  <https://decision-cli.dev/ns/session/init-001>
      prov:wasDerivedFrom ?source ;
      dec:definitionHash  ?hash ;
      dec:ontologyVersion ?version .
} LIMIT 1";
    match store.query(prov_q) {
        Ok(QueryResults::Solutions(mut sols)) => match sols.next() {
            Some(Ok(sol)) => {
                let source = sol
                    .get("source")
                    .map(term_literal_string)
                    .unwrap_or_default();
                let hash = sol
                    .get("hash")
                    .map(term_literal_string)
                    .unwrap_or_default();
                let ver = sol
                    .get("version")
                    .map(term_literal_string)
                    .unwrap_or_default();
                (source, hash, ver)
            }
            _ => (String::new(), String::new(), String::new()),
        },
        _ => (String::new(), String::new(), String::new()),
    }
}

#[allow(clippy::too_many_arguments)]
fn print_report(
    dec_dir: &Path,
    stream_iri: &str,
    stream_name: &str,
    action_iri: &str,
    authorized_goals: &[String],
    source: &str,
    hash: &str,
    ontology_version: &str,
) {
    let action_label = shorten_value_action(action_iri);
    let provenance_label = if bundled::lookup_value_action(action_iri).is_some() {
        format!("bundled, ontology v{ontology_version}")
    } else {
        format!("custom, ontology v{ontology_version}")
    };
    let hash_short = if hash.len() >= 12 {
        format!("sha256:{}…", &hash[..12])
    } else if !hash.is_empty() {
        format!("sha256:{hash}")
    } else {
        "sha256:(unknown)".to_string()
    };
    let display_name = if stream_name.is_empty() {
        shorten_stream(stream_iri)
    } else {
        stream_name.to_string()
    };
    let store_display = dec_dir.join("store").to_string_lossy().into_owned();

    println!("Value Stream:      {display_name}");
    if source.is_empty() {
        println!("Definition:        ({hash_short})");
    } else {
        println!("Definition:        {source} ({hash_short})");
    }
    println!("  full hash:       sha256:{hash}");
    println!("Terminal Value:    {action_label} ({provenance_label})");
    println!(
        "Authorized Goals:  {}",
        if authorized_goals.is_empty() {
            "(none)".to_string()
        } else {
            authorized_goals.join(", ")
        }
    );
    println!("Graph Store:       {store_display}");
}

fn term_iri_string(t: &oxigraph::model::Term) -> String {
    match t {
        oxigraph::model::Term::NamedNode(n) => n.as_str().to_string(),
        other => other.to_string(),
    }
}

fn term_literal_string(t: &oxigraph::model::Term) -> String {
    match t {
        oxigraph::model::Term::Literal(lit) => lit.value().to_string(),
        other => other.to_string(),
    }
}

fn shorten_value_action(iri: &str) -> String {
    let prefix = "https://decision-cli.dev/ns/value-actions/";
    if let Some(local) = iri.strip_prefix(prefix) {
        format!("va:{local}")
    } else {
        iri.to_string()
    }
}

fn shorten_stream(iri: &str) -> String {
    let prefix = "https://decision-cli.dev/ns/streams/";
    if let Some(local) = iri.strip_prefix(prefix) {
        local.to_string()
    } else if let Some(local) = iri.strip_prefix("stream:") {
        local.to_string()
    } else {
        iri.to_string()
    }
}
