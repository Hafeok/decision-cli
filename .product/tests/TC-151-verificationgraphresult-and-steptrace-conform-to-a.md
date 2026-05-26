---
id: TC-151
title: VerificationGraphResult and StepTrace conform to ADR-028 SHACL shapes
type: scenario
status: passing
validates:
  features:
  - FT-097
  adrs: []
phase: 1
runner: cargo-test
runner-args: tc_151_verificationgraphresult_and_steptrace_conform_to_a
runner-timeout: 120
last-run: 2026-05-26T13:12:58.060493885+00:00
last-run-duration: 0.6s
---

## Claim

Round-tripping a representative `dec:VerificationGraphResult` (and its inner `dec:VerificationStepTrace`s) through `StreamWriter` succeeds when the artifact is well-formed, and is rejected with a `SchemaViolation` error when it violates any of the FT-097 SHACL constraints (length parity, step-IRI membership, verdict-vs-trace consistency, rationale minLength).

## Scenarios

### Scenario A — well-formed result is accepted

Construct a `VerificationGraphResult` for a 3-step fixture graph (`VG-FIXTURE-001`) with:
- `dec:stepTraces` of length exactly 3, each `dec:tracesStep` referencing a step IRI that exists in `VG-FIXTURE-001`.
- All step outcomes `"pass"`.
- `dec:verdict = "approved"`.
- `dec:rationale = "all 3 steps passed; 2 TCs received pass evidence"` (≥ 20 chars).
- `prov:wasGeneratedBy`, `prov:wasAttributedTo`, `dcterms:created` populated.

Persist through `StreamWriter`. Expectation: write returns `Ok(_)`, the file appears at `.dec/verify/result/VGR-NNN.ttl`, and a re-read produces a structurally identical artifact (modulo blank-node renaming on `EvidenceProjection` nodes).

### Scenario B — length parity violation is rejected

Same fixture as A, but emit only 2 `stepTraces` for the 3-step graph. Expectation: `StreamWriter` returns `Err(SchemaViolation { shape: "VerificationGraphResultShape", path: "dec:stepTraces" })`; no file is written.

### Scenario C — unknown step IRI is rejected

Same fixture as A, but one `dec:tracesStep` points at an IRI that does not exist in `VG-FIXTURE-001`. Expectation: `StreamWriter` returns `Err(SchemaViolation { shape: "VerificationStepTraceShape", path: "dec:tracesStep" })`; no file is written.

### Scenario D — verdict-vs-trace inconsistency is rejected

Same fixture as A, but one step trace has `outcome = "fail"` with `dec:providesEvidenceFor` non-empty on the parent step, while `dec:verdict = "approved"`. Expectation: `StreamWriter` returns `Err(SchemaViolation { shape: "VerificationGraphResultShape", path: "dec:verdict" })` from the `sh:sparql` constraint that re-asserts the per-graph rule; no file is written.

### Scenario E — short rationale is rejected

Same fixture as A, but `dec:rationale = "ok"` (3 chars). Expectation: `StreamWriter` returns `Err(SchemaViolation { shape: "VerificationGraphResultShape", path: "dec:rationale" })` from the `sh:minLength` constraint matching [ADR-018](ADR-018)'s rationale rule; no file is written.

## Runner

`cargo test` against the new artifact-types unit test in `crates/decision-cli/src/core/ontology/verification_result_tests.rs` (or sibling). The test must construct fixture `VerificationGraph` artifacts in an in-memory store, then call `StreamWriter::write` with each scenario's payload and assert on the returned `Result`. The fixture graph and env are seeded once per test module; no external services.

## Non-goals

- Validating the runner that *produces* these artifacts (FT-098's TCs cover that).
- Validating the CLI rendering of these artifacts (FT-099's TCs cover that).
- Validating PROV-O chain inclusion beyond the fields declared in this slice — full PROV-O chain integrity is asserted by [FT-075](FT-075).