//! TC-055 — `dec init` seeds the `ephemeral-cli` env idempotently.
//!
//! Validates: FT-035 · ADR-028.
//! Spec: `.product/tests/TC-055-dec-init-seeds-ephemeral-cli-env-idempotently.md`
//!
//! The four acceptance criteria collapse into one end-to-end test
//! covering: file presence, file content invariants, idempotency across
//! re-invocations (after the file already exists), reproducibility
//! across separate clean tempdirs, and orchestration-store projection.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use decision_cli::core::ontology::verification_env::{
    from_turtle, EPHEMERAL_CLI_ENV_FILENAME, EPHEMERAL_CLI_ENV_ID,
};
use decision_cli::init::{run as init_run, DefinitionSource};
use oxigraph::io::RdfFormat;
use oxigraph::sparql::QueryResults;
use oxigraph::store::Store;

static COUNTER: AtomicU64 = AtomicU64::new(0);

fn fresh_workdir(tag: &str) -> PathBuf {
    let mut base = env::temp_dir();
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    let counter = COUNTER.fetch_add(1, Ordering::Relaxed);
    base.push(format!(
        "decision-cli-tc055-{tag}-{}-{}-{}",
        std::process::id(),
        nanos,
        counter,
    ));
    fs::create_dir_all(&base).expect("create temp workdir");
    base
}

struct WorkdirGuard(PathBuf);

impl WorkdirGuard {
    fn new(tag: &str) -> Self {
        Self(fresh_workdir(tag))
    }
    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for WorkdirGuard {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

const STREAM_TTL: &str = include_str!(
    "../src/core/bundled/assets/streams/engineering-development.ttl"
);

const ENV_CLASS_IRI: &str = "https://decision-cli.dev/ns#VerificationEnvironment";
const EXPECTED_ENV_IRI: &str =
    "https://decision-cli.dev/ns/env/ENV-001-ephemeral-cli";

fn write_seed_definition(dir: &Path) -> PathBuf {
    let p = dir.join("stream.ttl");
    fs::write(&p, STREAM_TTL).expect("write seed ttl");
    p
}

fn run_init(workdir: &Path) {
    let seed = write_seed_definition(workdir);
    init_run(workdir, DefinitionSource::File(seed)).expect("dec init succeeds");
}

fn env_file_path(workdir: &Path) -> PathBuf {
    workdir
        .join(".dec")
        .join("verify")
        .join("env")
        .join(EPHEMERAL_CLI_ENV_FILENAME)
}

fn env_dir(workdir: &Path) -> PathBuf {
    workdir.join(".dec").join("verify").join("env")
}

#[test]
fn ephemeral_cli_env_seed_exists_after_init() {
    let tmp = WorkdirGuard::new("a");
    let workdir = tmp.path();
    // dec init refuses if `.dec/` already exists; ensure the tempdir is
    // empty (the fresh_workdir helper creates an empty dir, but a
    // re-used path could carry leftovers).
    let _ = fs::remove_dir_all(workdir.join(".dec"));

    run_init(workdir);

    let target = env_file_path(workdir);
    assert!(target.exists(), "{target:?} should exist after dec init");

    // File parses as Turtle and yields a VerificationEnvironment.
    let env = from_turtle(&target).expect("seed parses as Turtle");
    assert_eq!(env.id, EPHEMERAL_CLI_ENV_ID);
    assert_eq!(env.env_type, "ephemeral-tempdir");
    assert_eq!(env.safety_class.as_str(), "isolated");
    assert_eq!(
        env.allowed_ops,
        vec![
            "shell".to_string(),
            "filesystem".to_string(),
            "sparql-local".to_string()
        ]
    );
    assert!(
        env.setup.as_ref().map(|s| !s.is_empty()).unwrap_or(false),
        "dec:setup must be non-empty"
    );
    assert!(
        env.teardown.as_ref().map(|s| !s.is_empty()).unwrap_or(false),
        "dec:teardown must be non-empty"
    );
    assert!(env.endpoint.is_none(), "local env must not carry endpoint");
}

#[test]
fn seed_file_byte_content_is_stable_after_re_init_attempt() {
    let tmp = WorkdirGuard::new("a");
    let workdir = tmp.path();
    // dec init refuses if `.dec/` already exists; ensure the tempdir is
    // empty (the fresh_workdir helper creates an empty dir, but a
    // re-used path could carry leftovers).
    let _ = fs::remove_dir_all(workdir.join(".dec"));

    run_init(workdir);

    let target = env_file_path(workdir);
    let bytes_before = fs::read(&target).expect("seed bytes after first init");

    // Re-running `dec init` against an already-initialised dir is
    // intentionally rejected (`InitError::AlreadyInitialised`), but the
    // existing seed must remain byte-identical.
    let seed = write_seed_definition(workdir);
    let _ = init_run(workdir, DefinitionSource::File(seed));

    let bytes_after = fs::read(&target).expect("seed bytes after re-init attempt");
    assert_eq!(
        bytes_before, bytes_after,
        "seed file must not be modified after re-init"
    );

    // Only one env file should exist under .dec/verify/env/.
    let count = fs::read_dir(env_dir(workdir))
        .expect("env dir readable")
        .filter_map(Result::ok)
        .filter(|e| {
            e.path()
                .extension()
                .map(|x| x == "ttl")
                .unwrap_or(false)
        })
        .count();
    assert_eq!(count, 1, "exactly one env file must exist under .dec/verify/env/");
}

#[test]
fn seed_is_reproducible_across_clean_tempdirs() {
    let a = WorkdirGuard::new("repro-a");
    let b = WorkdirGuard::new("repro-b");
    let _ = fs::remove_dir_all(a.path().join(".dec"));
    let _ = fs::remove_dir_all(b.path().join(".dec"));
    run_init(a.path());
    run_init(b.path());

    let bytes_a = fs::read(env_file_path(a.path())).expect("seed bytes a");
    let bytes_b = fs::read(env_file_path(b.path())).expect("seed bytes b");
    assert_eq!(
        bytes_a, bytes_b,
        "seed file bytes must be identical across clean tempdirs"
    );
}

#[test]
fn store_projection_contains_exactly_one_env() {
    let tmp = WorkdirGuard::new("a");
    let workdir = tmp.path();
    // dec init refuses if `.dec/` already exists; ensure the tempdir is
    // empty (the fresh_workdir helper creates an empty dir, but a
    // re-used path could carry leftovers).
    let _ = fs::remove_dir_all(workdir.join(".dec"));
    run_init(workdir);

    // Load the persisted store dump and run a SPARQL query against the
    // verify-env named graph.
    let dump_path = workdir.join(".dec").join("store").join("orchestration.nq");
    let bytes = fs::read(&dump_path).expect("read store dump");
    let store = Store::new().expect("in-memory store");
    store
        .load_from_reader(RdfFormat::NQuads, bytes.as_slice())
        .expect("load store dump");

    let q = format!(
        "SELECT ?s WHERE {{ GRAPH ?g {{ ?s a <{cls}> }} }}",
        cls = ENV_CLASS_IRI
    );
    let res = store.query(q.as_str()).expect("query store");
    let QueryResults::Solutions(sols) = res else {
        panic!("expected solutions");
    };
    let mut found: Vec<String> = Vec::new();
    for sol in sols {
        let sol = sol.expect("solution");
        if let Some(oxigraph::model::Term::NamedNode(n)) = sol.get("s") {
            found.push(n.as_str().to_string());
        }
    }
    assert_eq!(
        found.len(),
        1,
        "store must project exactly one VerificationEnvironment; found: {found:?}"
    );
    assert_eq!(found[0], EXPECTED_ENV_IRI);
}
