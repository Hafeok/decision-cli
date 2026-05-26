//! TC-173 — `[features] default-acknowledged-cross-cutting` clears per-feature
//! preflight gaps without requiring a per-feature `adrs:` link.
//!
//! Validates: FT-104.
//! Spec: `.product/tests/TC-173-default-acknowledged-cross-cutting-clears-per-feat.md`.
//!
//! The FT-104 cross-cutting algorithm is implemented in
//! `decision_cli::default_ack::evaluate_cross_cutting`. This integration
//! test stages a temp workdir with a synthetic `product.toml`, parses
//! it via [`load_default_acknowledge`], and drives the algorithm
//! through each of the spec's five scenarios — A through E — to assert
//! that the precedence rules and config-driven defaults match.
//!
//! The feature spec is explicit that this slice's behavior is a pure
//! function of `(cross-cutting ADRs, feature `adrs:`, feature
//! `adrs-rejected:`, config)`, so the test drives the algorithm
//! directly rather than shelling out to a subprocess. The product-cli
//! cross-repo implementation mirrors this same shape (FT-104 §Description).

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use decision_cli::default_ack::{
    evaluate_cross_cutting, load_default_acknowledge, CoverageStatus,
};

static COUNTER: AtomicU64 = AtomicU64::new(0);

fn tempdir(tag: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let mut base = std::env::temp_dir();
    base.push(format!(
        "decision-cli-tc173-{tag}-{}-{nanos}-{n}",
        std::process::id()
    ));
    fs::create_dir_all(&base).expect("create temp workdir");
    base
}

fn write_product_toml(workdir: &PathBuf, body: &str) {
    fs::write(workdir.join("product.toml"), body).expect("write product.toml");
}

const ADR_CC: &str = "ADR-CC";

#[test]
fn tc_173_default_acknowledged_cross_cutting_clears_per_feat() {
    let workdir = tempdir("default-ack");

    // ----- Scenario A: baseline without default-acknowledge -----
    // No product.toml at all → empty config → behavior matches today.
    let cfg = load_default_acknowledge(&workdir);
    assert!(
        cfg.adrs.is_empty(),
        "scenario A: empty workdir → empty config; got {:?}",
        cfg
    );

    let rows = evaluate_cross_cutting(
        &[ADR_CC.into()],
        &[ADR_CC.into()], // FT-LINKED lists ADR-CC
        &[],
        &cfg,
    );
    assert_eq!(
        rows[0].status,
        CoverageStatus::Linked,
        "scenario A: FT-LINKED → no gap (linked)"
    );

    let rows = evaluate_cross_cutting(
        &[ADR_CC.into()],
        &[], // FT-UNLINKED does not list ADR-CC
        &[],
        &cfg,
    );
    assert_eq!(
        rows[0].status,
        CoverageStatus::Missing,
        "scenario A: FT-UNLINKED → gap (missing)"
    );
    assert!(
        rows[0].status.is_gap(),
        "scenario A: missing row counts as gap"
    );

    // ----- Scenario B: default-acknowledge clears the gap -----
    write_product_toml(
        &workdir,
        "[features]\ndefault-acknowledged-cross-cutting = [\"ADR-CC\"]\n",
    );
    let cfg = load_default_acknowledge(&workdir);
    assert!(
        cfg.acknowledges(ADR_CC),
        "scenario B: config recognises ADR-CC"
    );

    // FT-LINKED: explicit link still wins (no special annotation).
    let rows = evaluate_cross_cutting(
        &[ADR_CC.into()],
        &[ADR_CC.into()],
        &[],
        &cfg,
    );
    assert_eq!(
        rows[0].status,
        CoverageStatus::Linked,
        "scenario B: explicit link takes precedence over default-ack"
    );

    // FT-UNLINKED: gap is cleared by default-ack and tagged.
    let rows = evaluate_cross_cutting(
        &[ADR_CC.into()],
        &[],
        &[],
        &cfg,
    );
    assert_eq!(
        rows[0].status,
        CoverageStatus::DefaultAcknowledged,
        "scenario B: FT-UNLINKED now satisfied via default-acknowledged"
    );
    assert!(
        !rows[0].status.is_gap(),
        "scenario B: default-acknowledged row must NOT count as a gap"
    );
    assert_eq!(
        rows[0].status.severity_label(),
        "default-acknowledged",
        "scenario B: the renderer carries the `default-acknowledged` tag",
    );

    // ----- Scenario C: feature frontmatter unchanged -----
    // The algorithm is purely a function of its inputs; the test calls
    // it with the SAME `feature_linked_adrs = []` slice it used in
    // scenario A. The fact that scenario B's outcome differs proves the
    // change came from the config, not from a frontmatter mutation.
    let unmutated = evaluate_cross_cutting(
        &[ADR_CC.into()],
        &[], // identical to the scenario-A FT-UNLINKED input
        &[],
        &cfg,
    );
    assert_eq!(
        unmutated[0].status,
        CoverageStatus::DefaultAcknowledged,
        "scenario C: same frontmatter (no `adrs:` change) — the acknowledgment lives in config"
    );

    // ----- Scenario D: removing the entry restores the gap -----
    write_product_toml(&workdir, "[features]\n");
    let cfg = load_default_acknowledge(&workdir);
    assert!(
        !cfg.acknowledges(ADR_CC),
        "scenario D: removed entry → no longer default-acked"
    );
    let rows = evaluate_cross_cutting(
        &[ADR_CC.into()],
        &[],
        &[],
        &cfg,
    );
    assert_eq!(
        rows[0].status,
        CoverageStatus::Missing,
        "scenario D: removing the entry restores the missing gap"
    );

    // ----- Scenario E: empty list behaves as absent -----
    write_product_toml(
        &workdir,
        "[features]\ndefault-acknowledged-cross-cutting = []\n",
    );
    let cfg_empty = load_default_acknowledge(&workdir);
    assert!(
        cfg_empty.adrs.is_empty(),
        "scenario E: empty list parses to empty set"
    );

    // Compare the two preflight outputs row-by-row: absent key vs empty
    // list must produce identical results.
    write_product_toml(&workdir, "[features]\n");
    let cfg_absent = load_default_acknowledge(&workdir);
    let rows_empty = evaluate_cross_cutting(&[ADR_CC.into()], &[], &[], &cfg_empty);
    let rows_absent = evaluate_cross_cutting(&[ADR_CC.into()], &[], &[], &cfg_absent);
    assert_eq!(
        rows_empty, rows_absent,
        "scenario E: empty list is indistinguishable from absent key"
    );
    assert_eq!(rows_empty[0].status, CoverageStatus::Missing);

    fs::remove_dir_all(&workdir).ok();
}
