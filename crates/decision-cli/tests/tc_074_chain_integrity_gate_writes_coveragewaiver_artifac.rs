//! TC-074 — chain-integrity gate writes CoverageWaiver artifact and lets dispatch proceed.
//!
//! Validates: FT-047 · ADR-031.
//! Spec: `.product/tests/TC-074-chain-integrity-gate-writes-coveragewaiver-artifac.md`
//!
//! Acceptance:
//!   * a new `CoverageWaiver` is persisted at `.dec/verify/waivers/CW-NNN.ttl`,
//!   * the waiver carries `dec:waiverFor`, `dec:waiverReason`,
//!     `dec:uncoveredAtWaive`, `prov:wasAttributedTo`, `dcterms:created`,
//!   * the waiver is committed via StreamWriter (SHACL passes),
//!   * the implementer worker is invoked and the session is opened,
//!   * `prov:used <CW-NNN>` is recorded on the dispatch session,
//!   * a second waive call mints `CW-NNN+1`.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use decision_cli::core::verify::WaiverIntent;
use decision_cli::implement::{run as implement_run, ImplementArgs};
use decision_cli::init::{run as init_run, DefinitionSource};
use oxigraph::io::RdfFormat;
use oxigraph::sparql::QueryResults;
use oxigraph::store::Store;

const FEATURE_ID: &str = "FT-U";
const REASON: &str = "Doc-only feature; verification is review-based per ADR-NNN";

static COUNTER: AtomicU64 = AtomicU64::new(0);

#[test]
fn tc_074_chain_integrity_gate_writes_coveragewaiver_artifac() {
    if which("git").is_none() {
        eprintln!("TC-074: git not on PATH — skipping (worker dispatch needs git)");
        return;
    }
    if which("code-writer").is_none() {
        eprintln!("TC-074: code-writer not on PATH — skipping");
        return;
    }

    env::set_var("CODE_WRITER_STUB", "1");
    env::remove_var("CODE_WRITER_CMD");

    happy_path_writes_waiver_and_proceeds();
    second_call_mints_next_id();
}

fn happy_path_writes_waiver_and_proceeds() {
    let workdir = fresh_workdir("tc-074-happy");
    init_git_repo(&workdir);
    init_run(
        &workdir,
        DefinitionSource::Template("engineering-development".into()),
    )
    .expect("dec init");
    seed_product_fixture_with_uncovered_tcs(&workdir, FEATURE_ID, &["TC-T1", "TC-T2"]);

    let mut args = ImplementArgs::new(FEATURE_ID);
    args.product_root = Some(workdir.clone());
    args.waiver = Some(WaiverIntent::new(REASON));

    let outcome = implement_run(&workdir, &args).expect("dec implement with waiver");

    // Acceptance #1: waiver file written at .dec/verify/waivers/CW-NNN.ttl
    let waivers_dir = workdir.join(".dec/verify/waivers");
    let entries: Vec<_> = fs::read_dir(&waivers_dir)
        .expect("read waivers dir")
        .filter_map(Result::ok)
        .collect();
    assert_eq!(entries.len(), 1, "TC-074: expected exactly one waiver file");
    let waiver_file = &entries[0];
    let name = waiver_file.file_name().to_string_lossy().to_string();
    assert!(
        name.starts_with("CW-") && name.ends_with(".ttl"),
        "TC-074: waiver file name is wrong: {name}"
    );

    // Acceptance #2: file content carries the right fields.
    let ttl = fs::read_to_string(waiver_file.path()).expect("read waiver");
    assert!(ttl.contains("dec:CoverageWaiver"), "missing class: {ttl}");
    assert!(ttl.contains(FEATURE_ID), "missing feature id: {ttl}");
    assert!(ttl.contains(REASON), "missing reason: {ttl}");
    assert!(ttl.contains("TC-T2"), "missing uncovered TC: {ttl}");
    assert!(
        ttl.contains("prov:wasAttributedTo"),
        "missing attribution: {ttl}"
    );
    assert!(ttl.contains("dcterms:created"), "missing created: {ttl}");

    // Acceptance #3: outcome carries the waiver IRI.
    let waiver_iri = outcome
        .waiver_iri
        .as_ref()
        .expect("TC-074: outcome.waiver_iri must be Some");
    assert!(
        waiver_iri.starts_with("https://decision-cli.dev/ns/waiver/CW-"),
        "TC-074: waiver IRI shape wrong: {waiver_iri}"
    );

    // Acceptance #4: PROV-O `prov:used <CW-NNN>` recorded on the session.
    let store = load_orchestration_store(&workdir);
    let q = format!(
        r#"PREFIX prov: <http://www.w3.org/ns/prov#>
ASK {{ GRAPH ?g {{ <{}> prov:used <{}> }} }}"#,
        outcome.session_iri, waiver_iri,
    );
    match store.query(&q).expect("ask runs") {
        QueryResults::Boolean(true) => {}
        QueryResults::Boolean(false) => panic!(
            "TC-074: session {} missing prov:used <{}>",
            outcome.session_iri, waiver_iri
        ),
        _ => panic!("TC-074: ASK returned non-boolean result"),
    }

    // Acceptance #5: the orchestration store carries the waiver as a
    // `dec:CoverageWaiver` (SHACL passed and the StreamWriter committed).
    let q_class = format!(
        r#"PREFIX dec: <https://decision-cli.dev/ns#>
ASK {{ GRAPH ?g {{ <{waiver_iri}> a dec:CoverageWaiver }} }}"#,
    );
    match store.query(&q_class).expect("ask runs") {
        QueryResults::Boolean(true) => {}
        QueryResults::Boolean(false) => {
            panic!("TC-074: waiver {waiver_iri} missing in orchestration store");
        }
        _ => panic!("TC-074: ASK returned non-boolean result"),
    }

    let _ = fs::remove_dir_all(&workdir);
}

fn second_call_mints_next_id() {
    let workdir = fresh_workdir("tc-074-second");
    init_git_repo(&workdir);
    init_run(
        &workdir,
        DefinitionSource::Template("engineering-development".into()),
    )
    .expect("dec init");
    seed_product_fixture_with_uncovered_tcs(&workdir, FEATURE_ID, &["TC-T1", "TC-T2"]);

    let mut args = ImplementArgs::new(FEATURE_ID);
    args.product_root = Some(workdir.clone());
    args.waiver = Some(WaiverIntent::new(REASON));

    let outcome1 = implement_run(&workdir, &args).expect("first dispatch");
    let outcome2 = implement_run(&workdir, &args).expect("second dispatch");

    let iri1 = outcome1.waiver_iri.expect("first waiver");
    let iri2 = outcome2.waiver_iri.expect("second waiver");
    assert_ne!(
        iri1, iri2,
        "TC-074: second waive must mint a NEW id ({iri1} == {iri2})"
    );

    let waivers_dir = workdir.join(".dec/verify/waivers");
    let entries: Vec<_> = fs::read_dir(&waivers_dir)
        .expect("read waivers dir")
        .filter_map(Result::ok)
        .collect();
    assert_eq!(
        entries.len(),
        2,
        "TC-074: expected two waiver files after second dispatch"
    );

    let _ = fs::remove_dir_all(&workdir);
}

// ---------------------------------------------------------------------
// Fixture helpers.
// ---------------------------------------------------------------------

fn fresh_workdir(tag: &str) -> PathBuf {
    let mut base = env::temp_dir();
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    base.push(format!(
        "decision-cli-{tag}-{}-{}-{}",
        std::process::id(),
        nanos,
        n
    ));
    fs::create_dir_all(&base).expect("create temp workdir");
    base
}

fn init_git_repo(workdir: &Path) {
    run_ok(Command::new("git").arg("-C").arg(workdir).args(["init", "-q"]));
    run_ok(
        Command::new("git")
            .arg("-C")
            .arg(workdir)
            .args(["config", "user.email", "tc-074@decision-cli.test"]),
    );
    run_ok(
        Command::new("git")
            .arg("-C")
            .arg(workdir)
            .args(["config", "user.name", "tc-074"]),
    );
    run_ok(
        Command::new("git")
            .arg("-C")
            .arg(workdir)
            .args(["commit", "--allow-empty", "-q", "-m", "initial"]),
    );
}

fn run_ok(cmd: &mut Command) {
    let out = cmd.output().expect("spawn");
    assert!(
        out.status.success(),
        "command {cmd:?} failed: {}\n{}",
        out.status,
        String::from_utf8_lossy(&out.stderr)
    );
}

fn seed_product_fixture_with_uncovered_tcs(
    workdir: &Path,
    feature_id: &str,
    tcs: &[&str],
) {
    let product_dir = workdir.join(".product");
    let features_dir = product_dir.join("features");
    fs::create_dir_all(&features_dir).expect("create .product/features");
    fs::create_dir_all(product_dir.join("adrs")).expect("create .product/adrs");
    fs::create_dir_all(product_dir.join("tests")).expect("create .product/tests");
    fs::create_dir_all(product_dir.join("graph")).expect("create .product/graph");

    fs::write(
        product_dir.join("config.toml"),
        r#"name = "tc-074-fixture"
schema-version = "1"

[product]
responsibility = "throwaway fixture for TC-074"

[paths]
features = ".product/features"
adrs = ".product/adrs"
tests = ".product/tests"
graph = ".product/graph"
requests = ".product/requests.jsonl"

[prefixes]
feature = "FT"
adr = "ADR"
test = "TC"

[phases]
2 = "Phase 2"
"#,
    )
    .expect("write .product/config.toml");

    let mut body = String::new();
    body.push_str("---\n");
    body.push_str(&format!("id: {feature_id}\n"));
    body.push_str("title: TC-074 fixture feature\n");
    body.push_str("phase: 2\n");
    body.push_str("status: in-progress\n");
    body.push_str("depends-on: []\n");
    body.push_str("adrs: []\n");
    body.push_str("tests:\n");
    for t in tcs {
        body.push_str(&format!("- {t}\n"));
    }
    body.push_str("domains: []\n");
    body.push_str("domains-acknowledged: {}\n");
    body.push_str("---\n\n## Description\n\nTC-074 fixture.\n");
    fs::write(
        features_dir.join(format!("{feature_id}-tc-074-fixture-feature.md")),
        body,
    )
    .expect("write feature_spec");
}

fn load_orchestration_store(workdir: &Path) -> Store {
    let dump = workdir.join(".dec/store/orchestration.nq");
    let bytes = fs::read(&dump).expect("read orchestration.nq");
    let store = Store::new().expect("in-memory store");
    store
        .load_from_reader(RdfFormat::NQuads, bytes.as_slice())
        .expect("load orchestration");
    store
}

fn which(bin: &str) -> Option<PathBuf> {
    let path = env::var_os("PATH")?;
    for dir in env::split_paths(&path) {
        let c = dir.join(bin);
        if c.is_file() {
            return Some(c);
        }
    }
    None
}
