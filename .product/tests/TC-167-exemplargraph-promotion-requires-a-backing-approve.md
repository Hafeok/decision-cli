---
id: TC-167
title: ExemplarGraph promotion requires a backing approved VerificationGraphResult on the referenced VG
type: scenario
status: passing
validates:
  features:
  - FT-101
  adrs: []
phase: 1
runner: cargo-test
runner-args: tc_167_exemplargraph_promotion_requires_a_backing_approve
runner-timeout: 120
last-run: 2026-05-26T14:52:51.702363790+00:00
last-run-duration: 0.4s
---

## Claim

`dec catalog exemplar new` refuses to promote a `dec:VerificationGraph` to exemplar unless that VG has at least one `dec:VerificationGraphResult` with `verdict = "approved"` and `dec:resultOf = <the same VG>`. **An exemplar that has never passed is not exemplary.**

## Scenarios

### Setup

- Fresh `.dec/` initialised via `dec init`.
- Empty `.dec/catalog/exemplars/`.
- Seed env `ENV-001`, two graphs:
  - `VG-PROVEN` covering one TC with a trivial passing step.
  - `VG-UNPROVEN` of identical shape but **never executed** (no `VerificationGraphResult` exists for it).
- Run `dec verify graph run VG-PROVEN` (depends on FT-099 being implemented; the test must skip with a clear diagnostic if FT-099 is not yet landed). This produces `VGR-001` with `dec:resultOf = VG-PROVEN`, `dec:verdict = "approved"`.

### Scenario A — promotion of a proven VG succeeds

Invoke `dec catalog exemplar new EX-001 --graph VG-PROVEN --safety-class isolated --pattern-name "trivial-shell-pass" --rationale "Canonical minimal verification: one shell-command step that always exits 0. Useful as a smoke template for any new env."`. Assertions:

- Exit code: 0.
- File `.dec/catalog/exemplars/EX-001.ttl` exists with `dec:exemplarOf = VG-PROVEN`, `dec:basedOnApprovedResult = VGR-001`, `dec:appliesToSafetyClass = "isolated"`.
- Confirmation stdout includes a line `Promoted VG-PROVEN to exemplar (latest verdict: approved)`.

### Scenario B — promotion of an unproven VG is rejected

Invoke `dec catalog exemplar new EX-002 --graph VG-UNPROVEN --safety-class isolated --pattern-name "untested" --rationale "An attempt to promote a graph that has no result history; expected to fail."`. Assertions:

- Exit code: 1.
- Stderr contains `ExemplarNotProven` and references `VG-UNPROVEN`.
- Stderr names the latest verdict (or "no result yet" if no VGR exists) so the operator knows what to do.
- File `.dec/catalog/exemplars/EX-002.ttl` is **not** created.

### Scenario C — promotion of a previously-failing-now-passing VG succeeds

Construct `VG-INITIALLY-FAILING` such that its first run fails, then mutate the step to pass and run again. After the second (passing) result exists, invoke `dec catalog exemplar new EX-003 --graph VG-INITIALLY-FAILING --safety-class isolated --pattern-name "recovered" --rationale "Demonstrates that the latest verdict is what counts; prior failures do not block promotion once the graph has a passing result."`. Assertions:

- Exit code: 0.
- The exemplar is bound to the **latest** approved VGR (`VGR-003`, not `VGR-002` which was the failing one).
- `EX-003.dec:basedOnApprovedResult` resolves to a VGR whose `dec:verdict = "approved"` AND `dec:resultOf = VG-INITIALLY-FAILING`.

### Scenario D — rationale too short is rejected

Invoke `dec catalog exemplar new EX-004 --graph VG-PROVEN --safety-class isolated --pattern-name "short" --rationale "too short"`. Assertions:

- Exit code: 1.
- Stderr contains a SHACL violation referencing the 40-char rationale minimum.
- File not created.

## Runner

`bash tests/scripts/tc-167-exemplar-requires-proven.sh`. Temp `.dec/`, the test skips with a clear `SKIP: FT-099 not yet landed` diagnostic if `dec verify graph run` is not available — Scenarios A and C depend on actually executing a VG to produce the backing VGR. Scenario B's assertion (refuse-promotion-without-result) **is** runnable without FT-099 — the test must run that scenario independently.

## Non-goals

- Auto-deletion of an exemplar when its underlying VG is removed (the spec says the exemplar becomes an `OrphanedExemplar` warning; a separate TC could cover the orphan-detection verb, out of slice here).
- Cross-env exemplar applicability inference (the operator declares `--safety-class` explicitly; the system does not infer).