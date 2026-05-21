//! TC-073 — chain-integrity gate refuses dispatch when uncovered TCs and no waiver.
//!
//! Validates: FT-047 · ADR-031.
//! Spec: `.product/tests/TC-073-chain-integrity-gate-refuses-dispatch-when-uncover.md`
//!
//! Acceptance:
//!   * dispatch fails before invoking the implementer worker,
//!   * error chain mentions `Error::ChainIntegrity`, the feature id,
//!     the uncovered TC, the remediation hint, and the waiver hint,
//!   * no session is created in the orchestration store,
//!   * no `CoverageWaiver` artifact is written under `.dec/verify/waivers/`.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use decision_cli::implement::{run as implement_run, ImplementArgs};
use decision_cli::init::{run as init_run, DefinitionSource};
use oxigraph::io::RdfFormat;
use oxigraph::sparql::QueryResults;
use oxigraph::store::Store;

const FEATURE_ID: &str = "FT-U";

static COUNTER: AtomicU64 = AtomicU64::new(0);

#[test]
fn tc_073_chain_integrity_gate_refuses_dispatch_when_uncover() {
    let workdir = fresh_workdir("tc-073");
    init_run(
        &workdir,
        DefinitionSource::Template("engineering-development".into()),
    )
    .expect("dec init");
    seed_feature_with_uncovered_tcs(&workdir, FEATURE_ID, &["TC-T1", "TC-T2"]);

    // Snapshot the orchestration store size BEFORE the gate fires so we
    // can assert no session was opened.
    let store_size_before = orchestration_store_size(&workdir);

    let mut args = ImplementArgs::new(FEATURE_ID);
    args.product_root = Some(workdir.clone());
    let err = implement_run(&workdir, &args).expect_err(
        "TC-073: dispatch must fail when uncovered TCs and no waiver",
    );

    let chain = format!("{err:#}");

    // Acceptance #2: error mentions Error::ChainIntegrity.
    assert!(
        chain.contains("Error::ChainIntegrity"),
        "missing Error::ChainIntegrity: {chain}"
    );

    // Acceptance #3: error names the feature id.
    assert!(
        chain.contains(FEATURE_ID),
        "missing feature id {FEATURE_ID}: {chain}"
    );

    // Acceptance #4: error names the uncovered TC.
    assert!(
        chain.contains("TC-T2"),
        "missing uncovered TC TC-T2: {chain}"
    );

    // Acceptance #5: remediation hint suggests dec verify graph generate.
    assert!(
        chain.contains("dec verify graph generate"),
        "missing remediation hint: {chain}"
    );
    assert!(
        chain.contains("--environment"),
        "missing --environment hint: {chain}"
    );

    // Acceptance #6: waiver hint with the flag form.
    assert!(
        chain.contains("--waive-coverage"),
        "missing waiver hint: {chain}"
    );

    // Acceptance #7: no session created (orchestration store unchanged).
    let store_size_after = orchestration_store_size(&workdir);
    assert_eq!(
        store_size_before, store_size_after,
        "TC-073: orchestration store grew despite gate failure"
    );
    assert_no_session_for_feature(&workdir, FEATURE_ID);

    // Acceptance #8: no CoverageWaiver artifact written.
    let waivers_dir = workdir.join(".dec/verify/waivers");
    if waivers_dir.exists() {
        let entries: Vec<_> = fs::read_dir(&waivers_dir)
            .expect("read waivers dir")
            .filter_map(Result::ok)
            .collect();
        assert!(
            entries.is_empty(),
            "TC-073: waiver dir not empty: {entries:?}"
        );
    }

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

fn seed_feature_with_uncovered_tcs(workdir: &Path, feature_id: &str, tcs: &[&str]) {
    let features_dir = workdir.join(".product/features");
    fs::create_dir_all(&features_dir).expect("create .product/features");
    let mut body = String::new();
    body.push_str("---\n");
    body.push_str(&format!("id: {feature_id}\n"));
    body.push_str("title: TC-073 fixture\n");
    body.push_str("phase: 2\n");
    body.push_str("status: planned\n");
    body.push_str("tests:\n");
    for t in tcs {
        body.push_str(&format!("- {t}\n"));
    }
    body.push_str("---\n\nFixture for TC-073.\n");
    fs::write(features_dir.join(format!("{feature_id}-fixture.md")), body)
        .expect("write fixture feature_spec");
}

fn orchestration_store_size(workdir: &Path) -> u64 {
    let path = workdir.join(".dec/store/orchestration.nq");
    fs::metadata(&path).map(|m| m.len()).unwrap_or(0)
}

fn assert_no_session_for_feature(workdir: &Path, feature_id: &str) {
    let dump = workdir.join(".dec/store/orchestration.nq");
    if !dump.exists() {
        return;
    }
    let bytes = fs::read(&dump).expect("read store dump");
    let store = Store::new().expect("in-memory store");
    store
        .load_from_reader(RdfFormat::NQuads, bytes.as_slice())
        .expect("load store dump");
    let q = format!(
        r#"PREFIX dec: <https://decision-cli.dev/ns#>
ASK {{ ?s dec:featureId "{feature_id}" }}"#
    );
    match store.query(&q).expect("ask runs") {
        QueryResults::Boolean(false) => {}
        QueryResults::Boolean(true) => {
            panic!("TC-073: a session was opened for {feature_id} despite gate failure");
        }
        _ => panic!("TC-073: ASK returned non-boolean result"),
    }
}
