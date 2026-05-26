---
id: TC-178
title: cargo build and cargo test --workspace pass with crates/product-cli/ as a workspace member
type: scenario
status: unimplemented
validates:
  features:
  - FT-105
  adrs: []
phase: 1
---

## Claim

After the FT-105 absorption lands, `cargo build --workspace` and `cargo test --workspace` complete cleanly with `crates/product-cli/` as a workspace member. Both halves' existing test suites continue to pass without modification.

## Scenarios

### Setup

- A clean checkout of the absorbed workspace (post-FT-105 merge).
- Standard Rust toolchain (`rustc`, `cargo`) at the version declared in the workspace's `rust-toolchain.toml` (if present) or stable.

### Scenario A — workspace builds clean

Run `cargo build --workspace --all-targets`. Assertions:

- Exit code: 0.
- Stderr contains no `warning:` lines that are new (i.e. introduced by the absorption itself). Pre-existing warnings in either codebase are allowed to persist; new warnings indicate the absorption caused regression.
- Build artifacts produced under `target/debug/`:
  - `dec` binary (decision-cli).
  - `product` binary (deprecation shim from `crates/product-shim/`).
  - `product-cli` library artifacts.
- Build artifacts NOT produced: any duplicate `product` binary from product-cli's own `[[bin]]` declaration. (FT-105 §Phase 2 reconciles this — product-cli's binary target is suppressed or renamed when it lives in the workspace.)

### Scenario B — workspace test suite passes

Run `cargo test --workspace --all-targets`. Assertions:

- Exit code: 0.
- The test output lists tests from each workspace member: `oxi-events`, `product-cli`, `decision-cli`, `product-shim`.
- Total test count equals the sum of each crate's pre-absorption test count (no tests dropped silently).
- No test marked as `ignored` due to the absorption (any ignores must have predated and have a documented reason).

### Scenario C — cargo dependency reconciliation succeeds

Run `cargo tree --workspace --duplicates`. Assertions:

- Either: the duplicates output is empty (perfect reconciliation).
- Or: every duplicate is documented in a `KNOWN_DUPLICATES` list in the repo (e.g. `tests/known_duplicates.txt`) with a reason — typically a transitive dependency that two direct deps pin to incompatible major versions, where bumping either is out of scope.
- Drift (a new duplicate not in the list) → fail.

### Scenario D — release build succeeds

Run `cargo build --workspace --release`. Assertions:

- Exit code: 0.
- Binaries produced under `target/release/`.
- Release-mode warnings are gated by the same allow-list as debug.

### Scenario E — `cargo doc` builds clean

Run `cargo doc --workspace --no-deps`. Assertions:

- Exit code: 0.
- HTML output under `target/doc/decision_cli/`, `target/doc/product_cli/`, etc.
- No doc-comment broken-link warnings introduced by the absorption (pre-existing broken links allowed).

### Scenario F — Clippy is clean (or no new violations)

Run `cargo clippy --workspace --all-targets -- -D warnings`. Assertions:

- Exit code: 0 (treating warnings as errors).
- If the absorption introduces clippy violations, the slice halts and they are fixed in the same PR (per CLAUDE.md's `cargo clippy ... -D warnings` rule).

### Scenario G — `git log crates/product-cli/` preserves history

Run `git log --oneline crates/product-cli/ | wc -l`. Assertions:

- Output is ≥ the number of commits in the pre-absorption product-cli repo (per the snapshot taken at the absorption point).
- A specific known commit (the pre-absorption HEAD, captured in the PR description) appears in the output.

## Runner

`bash tests/scripts/tc-178-workspace-build-test.sh`. Runs the seven cargo invocations and the git assertion in sequence, exiting on the first failure with diagnostic output. Suitable for both local validation and CI gating.

## Non-goals

- Specific test count assertions (test counts vary as suites evolve; the rule is "≥ pre-absorption sum", not exact numbers).
- Performance of the build (out of slice).
- Cross-platform parity (the absorption is a code organisation change; if a platform was supported before, it must remain supported — a separate per-platform CI matrix gates that, not this TC).
- Clippy lint policy changes (use the existing policy, don't change it as a side effect of absorption).
