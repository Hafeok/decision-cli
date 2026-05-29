---
id: TC-223
title: Generated value-stream is byte-deterministic given identical .product/ inputs
type: invariant
status: unimplemented
validates:
  features:
  - FT-114
  adrs: []
observes:
- file
phase: 4
runner: cargo-test
runner-args: tc_223_generated_value_stream_is_deterministic
runner-timeout: 30
---

## Description

Generating the value-stream twice from the same `.product/`
must produce byte-identical `.ttl` so re-running `dec init` on
a bootstrapped repo yields no diff. Without determinism,
operators see spurious changes that cloud the audit trail of
"what did init actually change."

## Acceptance Criteria

Cargo test:

1. Compose a temp `.product/` with three TCs whose runners are
   `cargo-test`, `bash`, and `pytest` (one each).
2. Call the generator twice:
   `generate_value_stream(&product_root, "some-repo")` →
   `ttl_1` and `ttl_2`.
3. Assert `ttl_1 == ttl_2` (byte equality).
4. Snapshot `ttl_1` against
   `tests/fixtures/init-default-stream.ttl` so PR diffs
   surface any unintended shape drift.
5. Repeat with a `.product/` that has zero TCs; assert the
   generator still produces a valid `.ttl` (it'll have role
   catalog + bindings but no subscriptions) and that it's
   byte-stable across two calls.

Deterministic ordering is the load-bearing property — the
generator must sort subscription entries by runner name (or
some explicit canonical order) so HashMap iteration order
doesn't leak in.
