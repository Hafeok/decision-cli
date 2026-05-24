---
id: TC-063
title: dec verify graph new accepts empty steps and rejects dangling refs
type: exit-criteria
status: passing
validates:
  features: []
  adrs: []
phase: 2
runner: cargo-test
runner-args: -p decision-cli --test tc_063_dec_verify_graph_new_accepts_empty_steps_and_rejec
runner-timeout: 120
last-run: 2026-05-24T19:13:59.513272896+00:00
last-run-duration: 0.3s
---

## Description

[FT-041](FT-041)'s exit criterion: `dec verify graph new` creates an empty graph (valid at create time per [FT-036](FT-036)) and rejects dangling `verifies` or `environment` references with `DanglingRef`.

## Acceptance Criteria

1. **Empty graph happy path.** `dec verify graph new --verifies FT-001 --environment ENV-001-ephemeral-cli` (where both refs resolve) creates `.dec/verify/graph/VG-NNN-*.ttl`, exits 0, prints the minted id + path. The on-disk Turtle contains the header and an empty `dec:steps ()`.

2. **MCP parity.** `dec_verify_graph_new` with `{ verifies: "FT-001", environment: "ENV-001-ephemeral-cli" }` produces a structurally equivalent file.

3. **Dangling verifies.** `dec verify graph new --verifies FT-999 --environment ENV-001-ephemeral-cli` exits 1 with `Error::DanglingRef { ref: "FT-999", kind: "verifies" }`; no graph file is written.

4. **Dangling environment.** `dec verify graph new --verifies FT-001 --environment ENV-999` exits 1 with `Error::DanglingRef { ref: "ENV-999", kind: "environment" }`; no graph file is written.

5. **Verifies polymorphism.** `dec verify graph new --verifies TC-013 --environment ENV-001-ephemeral-cli` succeeds — `dec:verifies` accepts a TC id.

6. **Caller-supplied id collision.** Two invocations with `--id VG-007` and different `--verifies` cause the second to fail with `Error::DuplicateId { id: "VG-007" }`.

7. **No partial state on failure.** Any failing invocation leaves the store and `.dec/verify/graph/` directory exactly as they were before the call.

## Fixture

- Tempdir with `dec init` plus a real `FT-001` feature_spec (or a fixture FT/TC pair) authored in `.product/`.

## Out of scope

- Step authoring (TC-066, TC-067).
- Graph list / show (TC-064, TC-065).