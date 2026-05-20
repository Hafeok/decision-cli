//! FT-019 — `dec init` seeds the verifier role catalog entry.
//! Validates the FT-019 invariant set:
//!
//! - The store contains exactly one `dec:Role` with `dec:roleId = "verifier"`
//!   after init.
//! - The verifier role's output type IRI matches `dec:VerificationVerdict`
//!   (FT-020 / ADR-018).
//! - `core::role_catalog::lookup("verifier")` is total post-init —
//!   returns `Some(Role)` whose fields match the seed.

use std::env;
use std::fs;
use std::path::PathBuf;

use decision_cli::core::role_catalog::{
    self, VERIFICATION_VERDICT_IRI, VERIFIER_ROLE_ID, VERIFIER_ROLE_IRI,
};
use decision_cli::init::{run as init_run, DefinitionSource};
use oxigraph::io::RdfFormat;
use oxigraph::sparql::QueryResults;
use oxigraph::store::Store;

#[test]
fn dec_init_seeds_exactly_one_verifier_role() {
    let workdir = tempdir("ft-019-init-seed");

    init_run(
        &workdir,
        DefinitionSource::Template("engineering-development".into()),
    )
    .expect("dec init template");

    let store = load_store_from_dump(&workdir);

    // Exactly one dec:Role with roleId "verifier" exists.
    let count = scalar_int(
        &store,
        "PREFIX dec: <https://decision-cli.dev/ns#> \
         SELECT (COUNT(?r) AS ?n) WHERE { \
           { ?r a dec:Role ; dec:roleId \"verifier\" . } \
           UNION \
           { GRAPH ?g { ?r a dec:Role ; dec:roleId \"verifier\" . } } \
         }",
    );
    assert_eq!(
        count, 1,
        "expected exactly one verifier role in the orchestration store, found {count}"
    );

    // The verifier's output type is dec:VerificationVerdict.
    let q = format!(
        "PREFIX dec: <https://decision-cli.dev/ns#> \
         ASK {{ \
           {{ <{iri}> dec:roleOutputType <{verdict}> . }} \
           UNION \
           {{ GRAPH ?g {{ <{iri}> dec:roleOutputType <{verdict}> . }} }} \
         }}",
        iri = VERIFIER_ROLE_IRI,
        verdict = VERIFICATION_VERDICT_IRI,
    );
    assert!(
        ask(&store, &q),
        "verifier role must declare dec:roleOutputType <{VERIFICATION_VERDICT_IRI}>"
    );
}

#[test]
fn role_catalog_lookup_after_init_is_total() {
    let workdir = tempdir("ft-019-lookup");
    init_run(
        &workdir,
        DefinitionSource::Template("engineering-development".into()),
    )
    .expect("dec init template");
    let store = load_store_from_dump(&workdir);

    let role = role_catalog::lookup(&store, VERIFIER_ROLE_ID)
        .expect("lookup ok")
        .expect("verifier present in catalog post-init");
    assert_eq!(role.role_id, VERIFIER_ROLE_ID);
    assert_eq!(role.iri, VERIFIER_ROLE_IRI);
    assert_eq!(role.output_type, VERIFICATION_VERDICT_IRI);
    // Three input types per the FT-019 spec.
    assert_eq!(role.input_types.len(), 3);
    // Model binding is a non-empty string (slice-2 inline per ADR-020).
    assert!(!role.model_binding.is_empty());

    let unknown = role_catalog::lookup(&store, "no-such-role").expect("lookup ok");
    assert!(unknown.is_none());
}

#[test]
fn lookup_against_uninitialised_store_returns_none() {
    // FT-019 error-handling clause: lookup against an uninitialised store
    // returns None; callers (FT-021) raise their own structured error.
    let store = Store::new().expect("empty store");
    let role = role_catalog::lookup(&store, VERIFIER_ROLE_ID).expect("lookup ok");
    assert!(role.is_none());
}

fn load_store_from_dump(workdir: &PathBuf) -> Store {
    let dump = workdir.join(".dec").join("store").join("orchestration.nq");
    let bytes = fs::read(&dump).expect("read store dump");
    let store = Store::new().expect("store");
    store
        .load_from_reader(RdfFormat::NQuads, bytes.as_slice())
        .expect("load store");
    store
}

fn ask(store: &Store, q: &str) -> bool {
    match store.query(q).expect("ASK") {
        QueryResults::Boolean(b) => b,
        _ => panic!("expected ASK to return boolean"),
    }
}

fn scalar_int(store: &Store, q: &str) -> i64 {
    match store.query(q).expect("SELECT count") {
        QueryResults::Solutions(mut sols) => {
            let sol = sols
                .next()
                .expect("count returned no rows")
                .expect("count solution");
            let term = sol.get("n").expect("missing ?n");
            let oxigraph::model::Term::Literal(lit) = term else {
                panic!("expected literal count, got {term:?}");
            };
            lit.value().parse().expect("count literal not an integer")
        }
        _ => panic!("expected SELECT to return solutions"),
    }
}

fn tempdir(label: &str) -> PathBuf {
    let mut p = env::temp_dir();
    let pid = std::process::id();
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    p.push(format!("{label}-{pid}-{nonce}"));
    fs::create_dir_all(&p).expect("create tempdir");
    p
}
