---
id: TC-348
title: def-ready against live orchestration store with produced implementer feedback returns Done
type: scenario
status: unimplemented
validates:
  features:
  - FT-138
  adrs: []
phase: 1
runner: cargo-test
runner-args: --package decision-cli --test ft_138_def_ready_implementer_feedback live_store_open_feedback_returns_done
runner-timeout: 120
observes:
- exit-code
---

## Acceptance criteria

Integration test for [FT-138](FT-138)'s production `inspect_dor` wiring against a live orchestration store. Reproduces the FT-110 misfire shape (rejected VG + open implementer feedback + superseded covering-graph) and asserts the classifier returns `Done`.

### Conditions

Cargo integration test in `crates/decision-cli/tests/ft_138_def_ready_implementer_feedback.rs`. Uses a tempdir-backed `.product/` + `.dec/` fixture.

**Setup:**

1. Tempdir with `.product/features/FT-T348.md` (minimal feature spec linking TC-T348), `.product/tests/TC-T348.md` (with configured bash runner), and a `.dec/store/orchestration.nq` containing:
   - One `dec:Feedback` artifact: `feedbackClass = "defect"`, `lifecycleState = "produced"`, `targetRole = "implementer"`, `sourceArtifact = <https://decision-cli.dev/ns/tc/TC-T348>`.
   - One `dec:VerificationGraph` for FT-T348 that is `superseded` (sentinel succession, mirroring the `urn:dec:retired-stale-dogfood-...` case).
2. Construct the production `inspect_dor::ProductionInspector` against the tempdir workdir + product_root.

**Assertion:**

- `FeatureReadyPlanner::new(production_inspector).classify("FT-T348", "BNCH-002")` returns `Action::Done`.
- Critically: returns `Done`, not `DispatchVerifyGraphAuthor` (the pre-FT-138 misfire) and not a `Stuck` of any kind.

### Rationale

End-to-end behavioural lock: the live SPARQL query that `inspect_dor` runs to detect open implementer feedback returns true for this fixture, and that drives the classifier through the new row before it hits the VG-missing branch. Without this slice, the classifier hits VG-missing → DispatchVerifyGraphAuthor and the drive stalls on `dispatch did not change state` — the witnessed FT-110 failure.

### Exit codes

- `0` — classifier returns `Action::Done` per ADR-079.
- `1` — anything else; test prints the actual Action variant.

### Surface

`exit-code` — cargo integration test against a tempdir fixture of `.product/` + `.dec/`.
