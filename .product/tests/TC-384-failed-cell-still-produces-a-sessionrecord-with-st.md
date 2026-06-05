---
id: TC-384
title: Failed cell still produces a SessionRecord with status failed
type: scenario
status: passing
validates:
  features:
  - FT-146
  adrs: []
phase: 1
runner: cargo-test
runner-args: -p decision-cli --test ft_146_cluster_session_persist ft_146_failed_cell_still_persists_session_record
runner-timeout: 120
observes:
- graph
last-run: 2026-06-05T09:18:18.466844397+00:00
last-run-duration: 0.3s
---

## Description

Scenario test for [FT-146](FT-146) §Invariants: "Every cell dispatch produces exactly one `dec:SessionRecord`. Mechanical, succeeded, and failed cells all produce a record."

Bootstraps a workdir, persists a cluster run whose only cell is in the `Failed` state with `usage: None`, and asserts the persistence path still writes a complete SessionRecord through the SHACL chokepoint.

## Assertions

1. **Failed cell session lands**: `dec:cellStatus = "failed"` on the cell's IRI.
2. **Cluster outcome reflects the failure**: `dec:clusterOutcome = "cell_failed"` on the parent activity.
3. **`dec:usageSource = "unreported"`** — no usage block was reported.
4. The store accepts the write (SHACL chokepoint does not reject a no-usage SessionRecord).

Failure to write either quad fails the test. This is the PROV-O coverage invariant: failed cells must not silently disappear from the graph.

## Runner

`cargo-test` invocation of `ft_146_failed_cell_still_persists_session_record`.