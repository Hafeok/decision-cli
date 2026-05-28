//! TC-015 — Bootstrap session reachable from `ValueStream` via PROV-O.
//! Validates: FT-008, FT-009 · ADR-004.
//! Spec: .product/tests/TC-015-bootstrap-session-reachable-from-valuestream-via-p.md
//!
//! For every initialised orchestration store, both of these must hold:
//!
//!   1. `<dec:session/init-001>` is typed `dec:Session`.
//!   2. The active `dec:ValueStream` artifact has a
//!      `prov:wasGeneratedBy <dec:session/init-001>` triple — the
//!      bootstrap session is the activity that generated the stream.
//!
//! The store layer for slice 1 writes the orchestration graph as an
//! `orchestration.nq` dump on disk (FT-009). This test runs the
//! init pipeline against a fresh working directory and re-loads the
//! persisted dump into an in-memory store to verify the invariant.

use std::env;
use std::fs;
use std::path::PathBuf;

use decision_cli::init::{self, DefinitionSource, BOOTSTRAP_SESSION_IRI};
use oxigraph::io::RdfFormat;
use oxigraph::sparql::QueryResults;
use oxigraph::store::Store;

const ASK_TC_015: &str = r"
PREFIX dec:  <https://decision-cli.dev/ns#>
PREFIX prov: <http://www.w3.org/ns/prov#>
ASK {
  <https://decision-cli.dev/ns/session/init-001> a dec:Session .
  ?stream a dec:ValueStream ;
          prov:wasGeneratedBy <https://decision-cli.dev/ns/session/init-001> .
}
";

const NEG_TC_015: &str = r"
PREFIX dec:  <https://decision-cli.dev/ns#>
PREFIX prov: <http://www.w3.org/ns/prov#>
SELECT ?stream WHERE {
  ?stream a dec:ValueStream .
  FILTER NOT EXISTS {
    <https://decision-cli.dev/ns/session/init-001> a dec:Session .
    ?stream prov:wasGeneratedBy <https://decision-cli.dev/ns/session/init-001>
  }
}
";

#[test]
fn bootstrap_session_reachable_from_valuestream_via_provo() {
    let workdir = fresh_workdir("tc-015");

    let outcome = init::run(
        &workdir,
        DefinitionSource::Template("engineering-development".to_string()),
    )
    .expect("dec init succeeds against the bundled template");

    assert_eq!(outcome.session_iri, BOOTSTRAP_SESSION_IRI);

    let dump = outcome.store_dump_path.clone();
    assert!(
        dump.exists(),
        "FT-009 must persist the orchestration dump at {}",
        dump.display()
    );

    let bytes = fs::read(&dump).expect("read orchestration.nq dump");
    let store = Store::new().expect("in-memory store opens");
    store
        .load_from_reader(RdfFormat::NQuads, bytes.as_slice())
        .expect("orchestration dump reloads cleanly");

    // Invariant #1+#2: ASK in TC-015's spec returns true.
    match store.query(ASK_TC_015).expect("ask query runs") {
        QueryResults::Boolean(true) => {}
        QueryResults::Boolean(false) => {
            panic!(
                "TC-015 ASK returned false: bootstrap session is not reachable \
                 from the ValueStream via prov:wasGeneratedBy"
            );
        }
        _ => panic!("TC-015 ASK returned a non-boolean result"),
    }

    // Negative check: no ValueStream may be missing the PROV chain.
    let QueryResults::Solutions(sols) = store.query(NEG_TC_015).expect("negative audit query runs")
    else {
        panic!("negative audit query returned non-solutions");
    };
    let orphan_streams: Vec<String> = sols
        .filter_map(Result::ok)
        .filter_map(|sol| match sol.get("stream") {
            Some(oxigraph::model::Term::NamedNode(n)) => Some(n.as_str().to_string()),
            _ => None,
        })
        .collect();
    assert!(
        orphan_streams.is_empty(),
        "ValueStream artifacts without a PROV link to <init-001>: {orphan_streams:?}"
    );

    // Cleanup the temp working directory.
    let _ = fs::remove_dir_all(&workdir);
}

fn fresh_workdir(tag: &str) -> PathBuf {
    let mut base = env::temp_dir();
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    base.push(format!(
        "decision-cli-{tag}-{}-{}",
        std::process::id(),
        nanos
    ));
    fs::create_dir_all(&base).expect("create temp workdir");
    base
}
