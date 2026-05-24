---
id: TC-082
title: MCP generate-then-accept refuses stale proposals after candidate set changes
type: scenario
status: passing
validates:
  features: []
  adrs: []
phase: 2
runner: cargo-test
runner-args: tc_082_mcp_generate_then_accept_refuses_stale_proposals_a
runner-timeout: 120
last-run: 2026-05-24T19:14:10.271430129+00:00
last-run-duration: 0.4s
---

## Premise

A client calls `dec_verify_graph_generate { feature_id: "FT-M", environment_id: "ENV-1" }` and receives `proposal_token = T1`. Between this and `dec_verify_graph_accept`, another client writes a graph covering all of `FT-M`'s TCs in `ENV-1`. The first client now calls `dec_verify_graph_accept { proposal, proposal_token: T1 }`.

## Acceptance Criteria

- `accept` recomputes the matcher state and finds that `FT-M` is now `CompleteSingle` against a different graph than `proposal_token` was issued for.
- `accept` returns `Error::ProposalStale` (structured MCP error).
- No new graph is written.
- The error message suggests re-running `dec_verify_graph_generate`.

## Notes

The MCP two-call protocol is the only place [FT-049](FT-049) has to deal with concurrent state mutation. Refusing stale proposals (rather than silently overwriting) is the correct conservative behaviour at Level 3.