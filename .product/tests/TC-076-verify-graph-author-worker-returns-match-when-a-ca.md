---
id: TC-076
title: verify-graph-author worker returns Match when a candidate covers all TCs
type: exit-criteria
status: passing
validates:
  features: []
  adrs: []
phase: 2
runner: pytest
runner-args: workers/verify-graph-author/tests/test_tc_076_match.py
runner-timeout: 120
last-run: 2026-05-24T19:14:09.038273853+00:00
last-run-duration: 0.4s
---

## Premise

A synthetic `VerifyGraphAuthorInput` bundle is constructed for `FT-Q` with TCs `[T1, T2]`, target env `ENV-1`, step vocabulary the six seed kinds, and `candidate_graphs = [{ id: VG-K, verifies: FT-Q, covers: [T1, T2], step_summaries: [...] }]`. The worker is invoked with a Claude client mocked to return a structured `GraphProposal::Match { graph_id: "VG-K", rationale: "VG-K already covers both TCs through ..." }`.

## Acceptance Criteria

- The worker exits 0.
- stdout contains a single line of JSON parseable as `GraphProposal` with `kind == "match"`.
- `proposal.match.graph_id == "VG-K"`.
- `proposal.bundle_hash` echoes the input's `bundle_hash` exactly.
- The worker performs no filesystem writes outside stdout.
- No `Anthropic` API call escapes the mock.

## Notes

Validates the worker's match path — it can return `Match` when prompted with adequate candidates and does so within the strict output schema.