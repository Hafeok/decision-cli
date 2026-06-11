//! Integration test for FT-138 — TC-348.
//!
//! Exercises the full production `FeatureReadyPlanner` +
//! `inspect_dor::has_open_implementer_feedback` stack against a
//! tempdir-backed orchestration store containing a `produced`
//! implementer-targeted defect feedback. Reproduces the FT-110 misfire
//! shape (rejected VG + open implementer feedback + superseded covering
//! graph) and asserts the classifier returns `Done` per ADR-079.

use std::fs;
use std::path::PathBuf;

use decision_cli::core::drive::Action;
use decision_cli::core::drive::PlanContext;
use decision_cli::features::drive::inspect::ProductionInspector;
use decision_cli::features::ft_119_drive_def_ready::planner::FeatureReadyPlanner;

#[test]
fn live_store_open_feedback_returns_done() {
    // Temporarily hide `product` from PATH so preflight returns Clean.
    // The test is for the open-feedback check, not preflight integration.
    let old_path = std::env::var_os("PATH");
    std::env::set_var("PATH", "");

    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path();

    // 1. Create .product/ fixture.
    let product_dir = root.join(".product");
    let features_dir = product_dir.join("features");
    let tests_dir = product_dir.join("tests");
    let adrs_dir = product_dir.join("adrs");
    fs::create_dir_all(&features_dir).unwrap();
    fs::create_dir_all(&tests_dir).unwrap();
    fs::create_dir_all(&adrs_dir).unwrap();

    // Create minimal product.toml so product-cli commands work.
    fs::write(
        root.join("product.toml"),
        r#"[product]
name = "test-fixture"
version = "0.1.0"

[paths]
features = ".product/features"
adrs = ".product/adrs"
tests = ".product/tests"
"#,
    )
    .unwrap();

    // Minimal feature spec linking TC-T348.
    fs::write(
        features_dir.join("FT-T348-def-ready-implementer-feedback.md"),
        r#"---
id: FT-T348
status: in-progress
tests:
  - TC-T348
---

## Description
Integration fixture for TC-348.

## Functional Specification
Minimal spec.

## Out of scope
None.
"#,
    )
    .unwrap();

    // Minimal TC with bash runner configured.
    fs::write(
        tests_dir.join("TC-T348-open-feedback.md"),
        r#"---
id: TC-T348
runner: bash
runner-args: "true"
---

## Acceptance criteria
Minimal TC body.
"#,
    )
    .unwrap();

    // 2. Create .dec/ orchestration store with:
    //    - One defect feedback for TC-T348 (produced, implementer)
    //    - One superseded VG for FT-T348
    let dec_dir = root.join(".dec");
    let store_dir = dec_dir.join("store");
    let verify_dir = dec_dir.join("verify");
    let graph_dir = verify_dir.join("graph");
    fs::create_dir_all(&store_dir).unwrap();
    fs::create_dir_all(&graph_dir).unwrap();

    // Write orchestration.nq with feedback artifact.
    fs::write(
        store_dir.join("orchestration.nq"),
        r#"<urn:dec:feedback:tc348-open-defect> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <https://decision-cli.dev/ns#Feedback> <urn:dec:graph/orchestration> .
<urn:dec:feedback:tc348-open-defect> <https://decision-cli.dev/ns#feedbackClass> "defect" <urn:dec:graph/orchestration> .
<urn:dec:feedback:tc348-open-defect> <https://decision-cli.dev/ns#lifecycleState> "produced" <urn:dec:graph/orchestration> .
<urn:dec:feedback:tc348-open-defect> <https://decision-cli.dev/ns#targetRole> "implementer" <urn:dec:graph/orchestration> .
<urn:dec:feedback:tc348-open-defect> <https://decision-cli.dev/ns#sourceArtifact> <https://decision-cli.dev/ns/tc/TC-T348> <urn:dec:graph/orchestration> .
<urn:dec:feedback:tc348-open-defect> <https://decision-cli.dev/ns#sourceSession> "urn:dec:session/verify-graph-runner/VG-T348/1234" <urn:dec:graph/orchestration> .
<urn:dec:feedback:tc348-open-defect> <https://decision-cli.dev/ns#evidence> "step 1 produced outcome fail; expected exit 0, got 101" <urn:dec:graph/orchestration> .
<https://decision-cli.dev/ns/graph/VG-T348> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <https://decision-cli.dev/ns#VerificationGraph> <urn:dec:graph/verify> .
<https://decision-cli.dev/ns/graph/VG-T348> <https://decision-cli.dev/ns#verifies> <https://decision-cli.dev/ns/feature/FT-T348> <urn:dec:graph/verify> .
<https://decision-cli.dev/ns/graph/VG-T348> <https://decision-cli.dev/ns#supersededBy> <urn:dec:retired-stale-dogfood-sentinel> <urn:dec:graph/verify> .
"#,
    )
    .unwrap();

    // Write a minimal superseded VG on disk (the planner checks on-disk state).
    fs::write(
        graph_dir.join("VG-T348.ttl"),
        r#"@prefix dec: <https://decision-cli.dev/ns#> .

<https://decision-cli.dev/ns/graph/VG-T348>
    a dec:VerificationGraph ;
    dec:verifies <https://decision-cli.dev/ns/feature/FT-T348> ;
    dec:bench <https://decision-cli.dev/ns/bench/BNCH-002> ;
    dec:steps () .
"#,
    )
    .unwrap();

    // 3. Construct production inspector + planner against the fixture.
    let ctx = PlanContext {
        workdir: PathBuf::from(root),
        product_root: PathBuf::from(root),
        env_override: None,
    };
    let inspector = ProductionInspector::new(&ctx);
    let planner = FeatureReadyPlanner::new(inspector);

    // 4. Classify FT-T348. Per ADR-079, open implementer feedback → Done
    //    before the VG-missing branch fires (the VG is superseded, so
    //    covering_graph_state returns Missing).
    let action = planner
        .classify("FT-T348", "BNCH-002")
        .expect("classification succeeds");

    assert_eq!(
        action,
        Action::Done,
        "expected Done per ADR-079; got {action:?}"
    );

    // Restore PATH.
    match old_path {
        Some(p) => std::env::set_var("PATH", p),
        None => std::env::remove_var("PATH"),
    }
}
