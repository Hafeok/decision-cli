---
id: FT-171
title: 'decision-cli: cluster_dispatch retries only failed cells — audit failure preserves the sandbox and re-dispatches the offending cell'
phase: 4
status: complete
depends-on:
- FT-170
adrs:
- ADR-080
tests:
- TC-433
- TC-434
- TC-435
- TC-436
domains: []
domains-acknowledged: {}
---

## Description

The cluster retry loop is all-or-nothing: when the coherence audit fails, the next round wipes the sandbox and re-dispatches **all** cells. Witnessed on FT-147: five of six cells succeeded every round, yet four rounds (~25 min, ~€0.56 each) were spent re-earning passed work because one cell kept failing the audit. Per-cell retry would have cut that session roughly 6×.

This slice makes audit failure preserve the sandbox and re-dispatch only the cells implicated by the failed audit checks, with the audit's diagnostic fed into the retried cell's prompt as corrective context.

## Functional Specification

### Inputs

- `cluster_dispatch::run` / `run_cells` and the `CoherenceAuditSpec` execution in `crates/decision-cli/src/features/drive/cluster_dispatch.rs`.
- Audit script stdout — each `FAIL check=<name>: <detail>` line (the existing convention in `scripts/checks/cluster-audit-*.py`).
- A `check → cell` mapping declared per TaskType (e.g. `shacl_shape`-prefixed checks implicate the `shacl_shape` cell), defaulting to "all cells" for unmapped checks so behaviour degrades to today's semantics, never silently narrower.

### Outputs

- On audit failure within the same `dec drive ship` round budget: the sandbox is **kept**; only implicated cells re-dispatch; their prompts carry an appended `### Prior audit failure` section quoting the FAIL lines; the audit re-runs after the repairs.
- Per-cell retry counter (cap: 2 retries per cell per cluster run) recorded on the FT-146 cell SessionRecords, so `dec session show` cost rollups attribute retries to cells.
- Full-wipe re-runs remain the fallback when a retried cell exhausts its cap.

### State

- FT-146 cluster SessionRecords gain accurate per-cell retry accounting (additional cell rows, same schema — one row per dispatch attempt, as today).

### Behaviour

1. Cells run in topo order (unchanged) → audit → on FAIL: parse FAIL lines → map to cells → re-dispatch those cells against the preserved sandbox (their upstream cell outputs intact) → re-audit.
2. A retried cell's downstream dependents re-run only if the retried cell's output content changed (hash compare), preserving the topo contract without redundant dispatches.
3. After the per-cell retry cap, the cluster run fails with the audit report — the operator (or the drive's outer loop) decides on a full re-run.

### Invariants

- A cell that passed and whose upstream outputs are unchanged is never re-dispatched within a cluster run.
- Audit always runs against a sandbox where every cell's declared output exists (composes with [FT-170](FT-170)'s placement guarantee).

### Error handling

- Unparseable audit output (no `FAIL check=` lines on a non-zero exit) falls back to today's whole-cluster failure with the raw output attached.

### Boundaries

- The audit scripts' check *content* is out of scope ([FT-172](FT-172)).
- The drive planner's outer iteration policy (max-iter) is unchanged.

## Out of scope

- Parallel cell dispatch (cells stay sequential in topo order).
- Cross-run sandbox reuse (each `dec drive ship` invocation still starts clean).