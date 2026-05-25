---
id: TC-122
title: per_type_shape_files_load_and_compose_universal_blocks
type: exit-criteria
status: failing
validates:
  features:
  - FT-072
  adrs: []
phase: 1
runner: cargo-test
runner-args: tc_122_per_type_shape_files_load_and_compose_universal_bl
runner-timeout: 120
last-run: 2026-05-25T23:43:28.428209899+00:00
last-run-duration: 0.4s
failure-message: "    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.16s\n     Running unittests src/lib.rs (target/debug/deps/decision_cli-5a57efa364e6b418)\n     Running unittests src/main.rs (target/debug/deps/dec-a1adbe2e30e8a6b1)\n     Running tests/anthropic_cache_breakpoint.rs (target/debug/deps/anthropic_cache_breakpoint-fe8cd0afdb6dea96)\n     Running tests/bundle_stakes.rs (target/debug/deps/bundle_stakes-3069bc6d56d5fdb6)\n     Running tests/capability_resolver.rs (target/debug/deps/capabi"
---

## Description

Exit criterion for FT-072: every per-type shape file loads at bootstrap, composes the universal mechanical fragment via `sh:and`, includes the BoundaryArtifact branch as the first `sh:or` alternative (where applicable), and the Rust and Python copies of the shape directory are byte-identical.

## Acceptance criteria

- `dec init` loads the full shape set without parse error.
- A test iterates the per-type shape files and asserts each `sh:NodeShape` with `sh:targetClass` composes `dec:MechanicalProvenanceShape` via `sh:and`.
- The same iterator asserts each non-Session, non-Dispatch shape's `sh:or` first branch matches `[ a sh:NodeShape ; sh:class dec:BoundaryArtifact ]`.
- A build-time diff between `crates/decision-cli/src/core/ontology/shapes/` (Rust source) and `workers/_shared/shapes/` (Python copy) reports no differences.
- The build-time range-agreement check (shared with TC-120) passes.

## Runner

`bash` script `tests/scripts/tc-122-shape-files-load-and-compose.sh` invoking both `cargo test` and a `diff -r` on the two shape directories.