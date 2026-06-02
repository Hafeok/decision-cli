---
id: TC-285
title: Every complete feature has at least four linked TCs
type: scenario
status: unimplemented
validates:
  features: []
  adrs:
  - ADR-072
phase: 1
runner: bash
runner-args: scripts/checks/feature-tc-coverage.sh
runner-timeout: 30
observes:
- exit-code
- stdout
---

## Purpose

Mechanical enforcement of **ADR-072** — every feature with `status: complete`
must have at least four linked TCs in its `tests:` front-matter block.

The check runs `scripts/checks/feature-tc-coverage.sh`, which scans
`.product/features/*.md`, counts entries under `tests:` for each feature, and
classifies under-floor cases:

- A pre-existing under-covered feature listed in
  `scripts/checks/feature-tc-coverage.baseline` is reported as
  `BASELINE:` (advisory, exit 0).
- A `status: in-progress` feature under the floor is reported as
  `WARNING:` (advisory, exit 0) — it will block once it transitions to
  `complete`.
- A `status: complete` feature under the floor and NOT in the baseline
  is reported as `ERROR:` (exit 1) — CI blocks.

This TC has empty `validates.features` by design: per ADR-014, cross-cutting
TCs validate every feature's implementation implicitly rather than naming a
specific feature.

## Acceptance

- Exit 0 when no new violation exists.
- Exit 1 when at least one new violation exists.
- Stdout enumerates ERROR / WARNING / BASELINE diagnostic lines so the
  failure record points at the offending feature IDs.

## Inputs

- `.product/features/*.md` — feature front-matter, specifically the `id`,
  `status`, and `tests:` block.
- `scripts/checks/feature-tc-coverage.baseline` — snapshot of features
  that were already under the floor at ADR-072 acceptance; one ID per
  line; `#` comments and blank lines ignored.
- Optional env overrides: `MIN_TC_COUNT` (default 4),
  `FEATURES_DIR`, `BASELINE_FILE`.

## Out of scope

- The recommended four coverage axes (happy / edge / integration /
  state) named in ADR-072 are a thinking aid for authors. The mechanical
  check counts entries; it does not classify them. Coverage-axis
  enforcement is deferred to a future ADR if and when the count rule is
  in steady state.
