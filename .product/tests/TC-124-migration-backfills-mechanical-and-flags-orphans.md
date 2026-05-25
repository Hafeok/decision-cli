---
id: TC-124
title: migration_backfills_mechanical_and_flags_orphans
type: exit-criteria
status: passing
validates:
  features:
  - FT-074
  adrs: []
phase: 1
runner: cargo-test
runner-args: tc_124_migration_backfills_mechanical_and_flags_orphans
runner-timeout: 120
last-run: 2026-05-25T20:40:44.484607360+00:00
last-run-duration: 0.3s
---

## Description

Exit criterion for FT-074: `dec migrate provenance --apply` on a fixture corpus with three artifact classes (conformant / backfillable / orphan) produces the correct verdict per artifact, backfills synthetic mechanical triples on the second class with `:isMigrationBackfill true`, and emits Feedback for the third class.

## Acceptance criteria

- Fixture corpus contains:
  - A conformant `:ADR` (both blocks present already).
  - A backfillable `:ADR` (existing front-matter `features:` list mapping to `:decidesFor` edges; no mechanical block).
  - An orphan `:Feature` (no mechanical block, no mappable informal edges).
- `dec migrate provenance --dry-run` reports three verdicts matching the expected classification.
- `dec migrate provenance --apply` writes synthetic `:HistoricalSession` + `:HistoricalAgent` for the backfillable artifact; the new artifact carries `prov:wasGeneratedBy` pointing at a session with `:isMigrationBackfill true`.
- A Feedback artifact of class `migration-orphan-needs-repair` is emitted for the orphan; the orphan artifact carries `:isMigrationOrphan true`.
- Re-running `--apply` is idempotent: no new triples, the same orphan Feedback (deduplicated by artifact ref).
- `dec migrate provenance cutover` exits 1 while orphans remain, succeeds and flips GraphWriter to reject-mode once orphans are resolved (test simulates resolution via direct annotation removal).

## Runner

`bash` script `tests/scripts/tc-124-migration-end-to-end.sh` orchestrating fixture setup, dry-run, apply, idempotence re-run, and cutover.