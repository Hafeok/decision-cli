---
id: TC-249
title: Bootstrap deactivates prior role-binding versions in the same transaction as new active write
type: invariant
status: unimplemented
validates:
  features:
  - FT-118
  adrs: []
observes:
- graph
phase: 4
runner: cargo-test
runner-args: tc_249_bootstrap_deactivates_prior_bindings
runner-timeout: 60
---

## Description

The uniqueness invariant must be preserved across bootstrap
writes. Without atomic prior-version deactivation, a brief
window exists where two active bindings coexist; a concurrent
reader (or a reader that runs immediately after the bootstrap
without retrying) trips the resolver's uniqueness check.

## Acceptance Criteria

Cargo test:

1. Seed a store with one active binding:
   `<binding/verify-graph-author/v1>` with `dec:active=true`,
   `dec:roleId="verify-graph-author"`.
2. Invoke `bootstrap_catalog::write_role_binding` with a new
   binding `<binding/verify-graph-author/v7>` marked active.
3. Assert in the post-write store:
   - The v7 binding is active (`dec:active=true`).
   - The v1 binding is inactive (`dec:active=false`).
   - The v1 binding's other quads (default_capability,
     roleId, etc.) are preserved unchanged.
   - The store has exactly ONE active binding for
     `roleId="verify-graph-author"`.
4. Read the transaction journal (or assert via a stub
   StreamWriter) that the v7 insert and v1 deactivation
   landed in the SAME transaction. (If a fault is injected
   between the two writes, BOTH roll back.)
