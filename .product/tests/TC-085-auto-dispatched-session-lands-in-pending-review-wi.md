---
id: TC-085
title: auto-dispatched session lands in pending_review without persisting graph
type: scenario
status: unimplemented
validates:
  features: []
  adrs: []
phase: 1
---

## Premise

The subscription fires for `(FT-J, ENV-1)`. The orchestrator picks up the event, assembles the bundle, invokes the (mocked) worker which returns a `New` proposal.

## Acceptance Criteria

- A `dec:Session` artifact is created with:
  - `status = pending_review`,
  - `dec:proposalDocument = <JSON of the New proposal>`,
  - `dec:verifies = FT-J`,
  - `dec:environment = ENV-1`.
- **No `VerificationGraph` artifact is written.** `.dec/verify/graph/` is unchanged.
- `dec session list` shows the pending-review session.
- `dec session show <id>` renders the proposal payload.
- A subsequent `dec verify graph generate FT-J --environment ENV-1 --from-session <id> --accept` reads the proposal from the session and persists the graph through the standard write path.
- Chain-integrity gate ([FT-047](FT-047)) on `dec implement FT-J` continues to refuse dispatch while the proposal is pending — an unaccepted proposal does **not** count as coverage.

## Notes

Level-3 autonomy is preserved by construction: the subscription never persists graphs. Acceptance is always a human gesture (or an MCP call from a deliberate review agent). The chain gate enforces this — pending proposals do not satisfy coverage.
