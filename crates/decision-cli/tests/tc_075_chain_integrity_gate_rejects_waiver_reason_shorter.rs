//! TC-075 — chain-integrity gate rejects waiver reason shorter than minimum length.
//!
//! Validates: FT-047 · ADR-031.
//! Spec: `.product/tests/TC-075-chain-integrity-gate-rejects-waiver-reason-shorter.md`
//!
//! Acceptance:
//!   * dispatch fails with `Error::InvalidArgument { field: "waiver.reason" }`,
//!   * exit code is 2 (asserted via the gate's structured error type),
//!   * no `CoverageWaiver` is written,
//!   * the implementer is not invoked,
//!   * the message names the minimum length and rejects whitespace-only.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use decision_cli::core::verify::WaiverIntent;
use decision_cli::implement::{run as implement_run, ImplementArgs};
use decision_cli::init::{run as init_run, DefinitionSource};

const FEATURE_ID: &str = "FT-U";

static COUNTER: AtomicU64 = AtomicU64::new(0);

#[test]
fn tc_075_chain_integrity_gate_rejects_waiver_reason_shorter() {
    too_short_reason_rejected();
    whitespace_only_reason_rejected();
}

fn too_short_reason_rejected() {
    let workdir = fresh_workdir("tc-075-short");
    init_run(
        &workdir,
        DefinitionSource::Template("engineering-development".into()),
    )
    .expect("dec init");
    seed_feature_with_uncovered_tcs(&workdir, FEATURE_ID, &["TC-T1", "TC-T2"]);

    let mut args = ImplementArgs::new(FEATURE_ID);
    args.product_root = Some(workdir.clone());
    args.waiver = Some(WaiverIntent::new("too short"));

    let err = implement_run(&workdir, &args)
        .expect_err("TC-075: dispatch must fail with InvalidArgument when reason is too short");
    let chain = format!("{err:#}");

    assert!(
        chain.contains("Error::InvalidArgument"),
        "missing Error::InvalidArgument: {chain}"
    );
    assert!(
        chain.contains("waiver.reason"),
        "missing field name `waiver.reason`: {chain}"
    );
    assert!(
        chain.contains("16"),
        "error message must name the minimum length (16): {chain}"
    );

    assert_no_waiver_written(&workdir);
    let _ = fs::remove_dir_all(&workdir);
}

fn whitespace_only_reason_rejected() {
    let workdir = fresh_workdir("tc-075-ws");
    init_run(
        &workdir,
        DefinitionSource::Template("engineering-development".into()),
    )
    .expect("dec init");
    seed_feature_with_uncovered_tcs(&workdir, FEATURE_ID, &["TC-T1", "TC-T2"]);

    let mut args = ImplementArgs::new(FEATURE_ID);
    args.product_root = Some(workdir.clone());
    // 32 whitespace characters — exceeds the raw length floor but fails
    // the non-whitespace counting rule per FT-047 §Error handling.
    args.waiver = Some(WaiverIntent::new("                                "));

    let err = implement_run(&workdir, &args).expect_err(
        "TC-075: dispatch must fail with InvalidArgument when reason is whitespace-only",
    );
    let chain = format!("{err:#}");

    assert!(
        chain.contains("Error::InvalidArgument"),
        "missing Error::InvalidArgument: {chain}"
    );
    assert!(
        chain.contains("waiver.reason"),
        "missing field name `waiver.reason`: {chain}"
    );
    assert!(
        chain.contains("whitespace-only") || chain.contains("non-whitespace"),
        "error must surface the whitespace-only rejection: {chain}"
    );

    assert_no_waiver_written(&workdir);
    let _ = fs::remove_dir_all(&workdir);
}

// ---------------------------------------------------------------------
// Fixture helpers (same shape as TC-073).
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
    body.push_str("title: TC-075 fixture\n");
    body.push_str("phase: 2\n");
    body.push_str("status: planned\n");
    body.push_str("tests:\n");
    for t in tcs {
        body.push_str(&format!("- {t}\n"));
    }
    body.push_str("---\n\nFixture for TC-075.\n");
    fs::write(features_dir.join(format!("{feature_id}-fixture.md")), body)
        .expect("write fixture feature_spec");
}

fn assert_no_waiver_written(workdir: &Path) {
    let waivers_dir = workdir.join(".dec/verify/waivers");
    if !waivers_dir.exists() {
        return;
    }
    let entries: Vec<_> = fs::read_dir(&waivers_dir)
        .expect("read waivers dir")
        .filter_map(Result::ok)
        .collect();
    assert!(
        entries.is_empty(),
        "TC-075: waiver file unexpectedly written: {entries:?}"
    );
}
