---
id: TC-208
title: 'Vocab module rename: verify_env -> verify_bench compiles with renamed identifiers'
type: scenario
status: passing
validates:
  features:
  - FT-112
  adrs: []
observes:
- stdout
- exit-code
phase: 4
runner: bash
runner-args: tests/scripts/tc-208-vocab-rename-compiles.sh
runner-timeout: 180
last-run: 2026-05-29T13:41:32.311273790+00:00
last-run-duration: 0.3s
---

## Description

The vocab rename is the load-bearing first step: every other
call site depends on the renamed identifiers. Until the workspace
compiles cleanly with the new names, nothing else is verifiable.

## Acceptance Criteria

Bash test:

1. Run `cargo build --workspace` from the repo root; assert exit 0.
2. Run `grep -RnE 'IRI_DEC_ENV_PREFIX|IRI_DEC_VERIFICATION_ENVIRONMENT|IRI_DEC_ENV_TYPE|IRI_DEC_GRAPH_VERIFY_ENV|IRI_DEC_RAN_IN_ENVIRONMENT'` over `crates/decision-cli/src/`; assert zero matches (the old constant names are gone).
3. Run the same grep for `IRI_DEC_BENCH_PREFIX|IRI_DEC_VERIFICATION_BENCH|IRI_DEC_BENCH_TYPE|IRI_DEC_GRAPH_VERIFY_BENCH|IRI_DEC_RAN_ON_BENCH`; assert ≥ 1 match for each (the new names are present).

`IRI_DEC_LEDGER_ENVIRONMENT` in `auto_dispatch.rs` is excluded
from the search — that's the ledger axis, deliberately not
renamed per FT-112 §Outputs.