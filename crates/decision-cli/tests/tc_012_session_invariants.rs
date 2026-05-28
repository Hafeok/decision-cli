//! TC-012 — Every Session links bundle hash, model version, and stream via PROV-O.
//!
//! Validates: FT-011 · ADR-004, ADR-005.
//! Spec: `.product/tests/TC-012-session-links-bundle-hash-model-version-and-stream.md`
//!
//! The invariant is global: every `dec:Session` artifact in the
//! orchestration store **must** carry the full PROV-O chain — a bundle
//! reference whose `dec:contentHash` is reachable, a model reference
//! whose `dec:modelVersion` is reachable, and a `dec:inStream` link
//! resolving to a `dec:ValueStream`.
//!
//! We exercise the invariant against a freshly initialised store that
//! has been driven through one `dec implement` run (FT-013 / FT-011),
//! then run the negative audit query — it must return empty.

use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

use decision_cli::implement::{run as implement_run, ImplementArgs};
use decision_cli::init::{run as init_run, DefinitionSource};
use oxigraph::io::RdfFormat;
use oxigraph::sparql::QueryResults;
use oxigraph::store::Store;

#[test]
fn session_links_bundle_hash_model_version_and_stream_via_provo() {
    let workdir = tempdir("tc-012");

    // 1. Bootstrap.
    init_run(
        &workdir,
        DefinitionSource::Template("engineering-development".into()),
    )
    .expect("dec init template");

    // 2. Drive one implementer dispatch in stub mode so a non-bootstrap
    //    Session is minted.
    env::set_var("CODE_WRITER_STUB", "1");
    let worker_cmd = build_worker_cmd();
    env::set_var("CODE_WRITER_CMD", &worker_cmd);
    let args = ImplementArgs::new("FT-013");
    implement_run(&workdir, &args).expect("dec implement");

    // 3. Load the persisted store and audit every Session in it.
    let dump = workdir.join(".dec").join("store").join("orchestration.nq");
    let store = Store::new().expect("store");
    let bytes = std::fs::read(&dump).expect("read store dump");
    store
        .load_from_reader(RdfFormat::NQuads, bytes.as_slice())
        .expect("load store");

    // 4. The negative audit query MUST return no rows.
    let audit = r"PREFIX dec: <https://decision-cli.dev/ns#>
PREFIX prov: <http://www.w3.org/ns/prov#>
SELECT ?s WHERE {
  GRAPH ?g { ?s a dec:Session }
  FILTER NOT EXISTS {
    GRAPH ?g2 { ?s prov:used ?bundle }
    GRAPH ?g3 { ?bundle dec:contentHash ?h }
  }
}";
    let missing_hash = count(&store, audit);
    assert_eq!(
        missing_hash, 0,
        "every Session must reach a bundle with dec:contentHash"
    );

    let audit_model = r"PREFIX dec: <https://decision-cli.dev/ns#>
PREFIX prov: <http://www.w3.org/ns/prov#>
SELECT ?s WHERE {
  GRAPH ?g { ?s a dec:Session }
  FILTER NOT EXISTS {
    GRAPH ?g2 { ?s prov:used ?model }
    GRAPH ?g3 { ?model dec:modelVersion ?v }
  }
}";
    let missing_model = count(&store, audit_model);
    // The bootstrap session (init-001) intentionally has no
    // dec:modelRef — it predates the model catalogue. TC-015 covers
    // its PROV-O reachability. So we filter the bootstrap session out
    // explicitly for this leg of the invariant.
    let bootstrap_iri = "https://decision-cli.dev/ns/session/init-001";
    let q_filtered = format!(
        "PREFIX dec: <https://decision-cli.dev/ns#>
PREFIX prov: <http://www.w3.org/ns/prov#>
SELECT ?s WHERE {{
  GRAPH ?g {{ ?s a dec:Session }}
  FILTER (?s != <{bootstrap_iri}>)
  FILTER NOT EXISTS {{
    GRAPH ?g2 {{ ?s prov:used ?model }}
    GRAPH ?g3 {{ ?model dec:modelVersion ?v }}
  }}
}}"
    );
    let missing_model_excl = count(&store, q_filtered.as_str());
    assert!(
        missing_model_excl == 0,
        "every non-bootstrap Session must reach a model with dec:modelVersion (missing={missing_model_excl}, total-with-bootstrap={missing_model})"
    );

    // The ValueStream artifact lives in the default graph (FT-008
    // persistence path); `?stream a dec:ValueStream` must be matched
    // outside a `GRAPH` wrapper. The `dec:inStream` link is added by
    // StreamWriter under a named graph, so we still wrap that subquery
    // in `GRAPH ?g2`.
    let audit_stream = format!(
        "PREFIX dec: <https://decision-cli.dev/ns#>
SELECT ?s WHERE {{
  GRAPH ?g {{ ?s a dec:Session }}
  FILTER (?s != <{bootstrap_iri}>)
  FILTER NOT EXISTS {{
    GRAPH ?g2 {{ ?s dec:inStream ?stream }}
    {{ ?stream a dec:ValueStream }} UNION {{ GRAPH ?g3 {{ ?stream a dec:ValueStream }} }}
  }}
}}"
    );
    let missing_stream = count(&store, audit_stream.as_str());
    assert_eq!(
        missing_stream, 0,
        "every non-bootstrap Session must carry dec:inStream → dec:ValueStream"
    );
}

fn count(store: &Store, q: &str) -> usize {
    let results = store.query(q).expect("audit query");
    let QueryResults::Solutions(sols) = results else {
        panic!("expected solutions for audit query");
    };
    sols.count()
}

fn tempdir(label: &str) -> PathBuf {
    let mut p = env::temp_dir();
    let pid = std::process::id();
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    p.push(format!("{label}-{pid}-{nonce}"));
    std::fs::create_dir_all(&p).expect("create tempdir");
    p
}

fn build_worker_cmd() -> String {
    // Resolve the workspace's code-writer project so we run via uv.
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest
        .parent()
        .and_then(Path::parent)
        .expect("workspace root");
    let worker_dir = workspace_root.join("workers").join("code-writer");

    // Best-effort sync — uv is idempotent and fast on a warm cache.
    let _ = Command::new("uv")
        .arg("--project")
        .arg(&worker_dir)
        .arg("sync")
        .arg("--quiet")
        .status();
    format!("uv --project {} run code-writer", worker_dir.display())
}
