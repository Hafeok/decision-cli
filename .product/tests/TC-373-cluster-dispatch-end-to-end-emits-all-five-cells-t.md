---
id: TC-373
title: cluster_dispatch end-to-end emits all five cells then runs audit and commits
type: scenario
status: passing
validates:
  features:
  - FT-139
  adrs: []
phase: 1
runner: cargo-test
runner-args: --package decision-cli --test ft_139_cluster_dispatch_end_to_end cluster_dispatch_add_judge_worker_end_to_end
runner-timeout: 180
observes:
- exit-code
last-run: 2026-06-04T14:35:45.762132444+00:00
last-run-duration: 0.2s
---

## Acceptance criteria

End-to-end integration test for [FT-139](FT-139)'s `cluster_dispatch::run`. Verifies that a feature carrying `task_type: add-judge-worker` drives through cell emission, coherence audit, and finalize-commit without operator intervention. Locks in [ADR-080](ADR-080)'s cluster-atomicity invariant.

### Conditions

Cargo integration test in `crates/decision-cli/tests/ft_139_cluster_dispatch_end_to_end.rs` against a tempdir-backed workdir + product_root.

**Setup:**

1. Tempdir with a minimal `.product/` + `.dec/` initialised (use the existing helpers from FT-115's worktree machinery).
2. A feature_spec at `.product/features/FT-T373.md` with `task_type: add-judge-worker` and TC links matching the cluster's coherence-audit fixture.
3. A stub capability resolver that returns deterministic `endpoint` + `model_id` per cell (no live LiteLLM call; the LiteLLM client is mocked at the cell-dispatch boundary).
4. A stub `cell_executor` that emits canonical pass-fixtures for all five cells per the TaskType declaration.

**Action:**

- Call `cluster_dispatch::run(workdir, ctx, args { feature_id: "FT-T373" }, "add-judge-worker")`.

**Assertions:**

- Returns `Ok(ClusterOutcome::Done { cells_emitted: 5, audit_outcome: AuditOk })`.
- The worktree contains all five expected files (capability_binding, pydantic_io_models, system_prompt, agent_loop, unit_tests) at the paths the TaskType declares.
- A commit landed under `[FT-T373]` (assert via `git log`).
- The feature_spec's `status:` transitioned to `complete` (assert via product_core).
- The coherence audit ran (assert via a sentinel side-effect file the stub audit drops).
- Cell sessions emitted into the orchestration store (5 `dec:Session` artifacts, one per cell, with cluster id IRIs).

### Exit codes

- `0` — every assertion holds; cluster dispatched atomically.
- `1` — any assertion fails; test prints the actual `ClusterOutcome` for diagnosis.

### Surface

`exit-code` — cargo integration test against tempdir + stubs; mocks LiteLLM, does not call the live network.