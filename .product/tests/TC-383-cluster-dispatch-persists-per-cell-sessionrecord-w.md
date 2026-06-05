---
id: TC-383
title: cluster_dispatch persists per-cell SessionRecord with token breakdown
type: exit-criteria
status: passing
validates:
  features:
  - FT-146
  adrs: []
phase: 1
runner: cargo-test
runner-args: -p decision-cli --test ft_146_cluster_session_persist ft_146_cluster_dispatch_persists_session_records_with_token_breakdown
runner-timeout: 180
observes:
- graph
last-run: 2026-06-05T09:18:18.466844397+00:00
last-run-duration: 10.0s
---

## Description

End-to-end exit-criteria test for [FT-146](FT-146).

Bootstraps a workdir via `dec init` against the engineering-development stream, then calls `core::graph::cluster_session::persist_cluster_run` with one mechanical cell and one LLM-backed Scaleway cell whose `WorkerResponseUsage` reports base=3210, cache_write=0, cache_hit=0, output=987. Round-trips through `StreamWriter::commit` (the FT-057 SHACL chokepoint) and persists the store back to disk.

## Assertions

Re-loads the orchestration store from the dump and runs SPARQL against it:

1. **Cluster activity outcome**: `cluster_iri dec:clusterOutcome "succeeded"`.
2. **Cluster activity class**: `cluster_iri a dec:ClusterDispatch`.
3. **LLM cell token-breakdown**: the four FT-057 predicates on the agent_loop cell's `dec:SessionRecord` carry exactly the worker-reported values (`input_tokens_base=3210`, `input_tokens_cache_write=0`, `input_tokens_cache_hit=0`, `output_tokens=987`).
4. **LLM cell `dec:usageSource`** = `"worker-reported"`.
5. **LLM cell `dec:cellStatus`** = `"succeeded"`.
6. **Mechanical cell token-breakdown**: all four predicates = 0; `dec:usageSource = "unreported"`; `dec:cellStatus = "mechanical"`.

Failure of any assertion fails the test. The SHACL chokepoint must accept every write (Scaleway endpoint with zero cache fields passes FT-057's `check_scaleway_no_cache`).

## Runner

`cargo-test` invocation of the integration test at
`crates/decision-cli/tests/ft_146_cluster_session_persist.rs::ft_146_cluster_dispatch_persists_session_records_with_token_breakdown`.