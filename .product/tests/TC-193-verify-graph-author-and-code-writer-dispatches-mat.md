---
id: TC-193
title: verify-graph-author and code-writer dispatches materialize as dec:Session artifacts visible in dec session list
type: exit-criteria
status: passing
validates:
  features:
  - FT-109
  adrs:
  - ADR-004
phase: 3
runner: cargo-test
runner-args: tc_193_worker_dispatch_sessions_materialize
runner-timeout: 60
last-run: 2026-05-27T10:44:23.462262169+00:00
last-run-duration: 0.6s
---

## Claim

After `verify_graph_generate::run_generate` dispatches the verify-graph-author worker (FT-107), a `dec:Session` artifact exists in the orchestration store whose IRI matches the dispatch's `receivingSession` IRI on emitted/transitioned feedback. Same for `implement::run` and the code-writer. The session carries `rdf:type dec:Session`, `prov:startedAtTime`, `dec:roleId`, `dec:status`, `dec:featureId`, and `dec:inStream`.

## Scenarios

### Setup A (verify-graph-author)

- Feature `FT-T193a` with one TC, one pre-seeded defect feedback (so the FT-107 matcher gate falls through).
- A stubbed verify-graph-author returning a valid `New` proposal that cites the seeded feedback.

### Test A

Call `verify_graph_generate::run_generate` with mode `Accept`. After it returns, query the store for `?s rdf:type dec:Session ; dec:roleId "verify-graph-author"`. Assert:

1. Exactly one session of that role exists for this dispatch.
2. The session IRI equals the `dec:receivingSession` literal on the cited feedback (post-transition).
3. Session has `dec:status = "completed"`, `dec:featureId = "FT-T193a"`, non-empty `prov:startedAtTime`.

### Setup B (code-writer)

- Feature `FT-T193b` with one TC, one pre-seeded implementer-targeted defect feedback.
- Mock code-writer (via FT-108's `install_mock`) returning a `CodeChange` citing the feedback.

### Test B

Call `implement::run`. After it returns, query the store for sessions with `dec:roleId "implementer"`. Assert symmetric structure (matches `receivingSession` on the transitioned feedback, status="completed", feature_id set).

### Boundary

- A failing dispatch (worker returns error) transitions the session to `dec:status "failed"` instead of `"completed"`.