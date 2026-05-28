---
id: FT-014
title: Code Structure and Quality Standards Enforcement
phase: 1
status: complete
depends-on:
- FT-015
adrs: []
tests:
- TC-042
- TC-043
- TC-044
- TC-045
- TC-086
domains: []
domains-acknowledged:
  ADR-012: ADR-012 (per-stream working directory discovery) governs CLI entry; FT-014 runs after the working directory is resolved and does not re-discover it.
  ADR-001: ADR-001 governs the oxi-events crate boundary; FT-014 does not cross or alter that boundary.
  ADR-027: ADR-027 (authority declarations in role catalog) is implemented by FT-030; FT-014 does not introduce or modify a role catalog entry.
  ADR-021: ADR-021 (action-interpretation agreement metric) is a Slice 2 fitness function implemented by FT-024; FT-014 produces no action/interpretation pair.
  ADR-023: ADR-023 (feedback class controlled vocabulary) is implemented by FT-028; FT-014 produces no feedback artifacts.
  ADR-004: ADR-004 (PROV-O) governs session/event lineage; FT-014 produces no new Session or event type and inherits lineage from the harness.
  ADR-018: ADR-018 (VerificationVerdict schema) is a Slice 2 artifact implemented by FT-020; FT-014 neither emits nor consumes verdicts.
  ADR-002: ADR-002 (graph-as-state) governs persistence semantics; FT-014 reads/writes via the GraphWriter chokepoint and does not introduce event-sourced state.
  ADR-022: ADR-022 (feedback as a first-class flow class) is a Slice 3 concern implemented by FT-026; FT-014 neither emits nor routes feedback.
  ADR-025: ADR-025 (blocking vs non-blocking feedback semantics) is implemented by FT-032; FT-014 has no feedback to gate.
  ADR-016: ADR-016 (vertical-slice + compile-time SDP) is migrated by FT-018; FT-014's code is reorganised under that migration, not by this feature.
  ADR-024: ADR-024 (feedback lifecycle state machine) is implemented by FT-027; FT-014 produces no feedback artifacts.
  ADR-017: ADR-017 (action-interpretation pairing) is a Slice 2 structural requirement implemented by FT-021; FT-014 is out of scope for the pairing.
  ADR-005: ADR-005 (value-stream scope) governs command-time scope; FT-014 runs inside an already-scoped command and does not introduce a new scope check.
---

## Description

Implement the four structural quality rules from ADR-013 — file length, function length, module decomposition, single-responsibility doc comments — as automated checks that run on every implementation through `product verify --platform`. Cover both implementation surfaces: the Rust workspace (`crates/oxi-events`, `crates/decision-cli`) and the Python workers (`workers/code-writer`).

The deliverable is a set of `scripts/checks/` enforcement scripts plus a small set of cross-cutting TCs that wire them into the verify pipeline. ADR-014 governs how those rules live in the graph (as cross-cutting ADRs and cross-cutting TCs); this feature is the mechanical implementation that makes the verify pipeline actually catch violations.

## Functional Specification

### Inputs

- The current contents of `crates/*/src/**/*.rs` and `workers/*/**/*.py` (excluding `tests/`, `benches/`, and any generated stubs).
- Optional environment variables that override default thresholds: `FILE_LENGTH_HARD`, `FILE_LENGTH_WARN`, `FN_LENGTH_HARD`, `FN_LENGTH_WARN`.
- The repository's `scripts/checks/` directory, which holds the enforcement scripts themselves.

### Outputs

- A non-zero exit code from any failing rule script (1 for hard limits, 2 for warning-only state, 0 for clean).
- Human-readable diagnostic lines on stdout listing each offending file/function with its measurement and the threshold it exceeded.
- Updated TC status in front-matter after `product verify --platform` runs (pass / fail / warn).

### State

No persistent state owned by this feature. The scripts are pure — they read the repository tree and exit. TC pass/fail state is owned by product-cli's verify pipeline.

### Behaviour

- `scripts/checks/file-length.sh` — counts lines in every first-party source file under `crates/*/src/` and `workers/*/` (excluding tests and benches); exit 1 if any exceeds the hard limit, exit 2 if any is in the warning band, exit 0 otherwise.
- `scripts/checks/function-length.sh` — scans every `*.rs` file under `crates/*/src/`; uses awk-based brace-depth tracking to count statement lines per function; emits ERROR/WARN lines and propagates the worst severity as exit code.
- `scripts/checks/function-length.py` — same contract for `workers/*/**/*.py`; uses `ast` to walk every `FunctionDef`/`AsyncFunctionDef` and count statement nodes.
- `scripts/checks/module-structure.sh` — asserts that each crate's `src/` declares the canonical top-level modules listed in ADR-013, and that `crates/decision-cli/src/main.rs` is at most 80 lines.
- `scripts/checks/single-responsibility.sh` — checks the first non-shebang line of each first-party source file. Rust must start with `//! `; Python must start with `"""`. Either must not contain the substring ` and `.

Each script is independently runnable from a developer laptop and from CI. Each respects the three-tier exit code model (0 clean / 1 error / 2 warning).

### Invariants

- A clean repository — every first-party source file under thresholds, every module structure correct, every doc comment present and single-responsibility — produces exit 0 from every check.
- A single offending file in any one rule produces exit 1 or 2 from the matching script and a clear, parseable diagnostic line naming the file and the measurement.
- The check scripts are deterministic over the same tree — same input, same output, same exit code, regardless of platform or invocation order.
- No check writes to the source tree or to `.product/` during execution. Checks are pure reads.

### Error handling

- A script that cannot find any matching source files (e.g. cloned-but-not-built worktree) exits 0 with a single `OK: no first-party source files found` line. This is the empty-tree case, not an error.
- A script whose required dependency is missing (`awk`, `python3`, `ripgrep`) exits a documented sentinel code (e.g. 127) and surfaces a precondition message. Missing deps are a CI environment problem, not a rule violation.
- Diagnostic messages are written to stdout (not stderr) so `product verify` captures them in the TC failure record. Errors specific to the *script itself* go to stderr.

### Boundaries

- This feature **does not** enforce SDP forbidden-words on `crates/oxi-events` doc comments. ADR-013 names that as deferred mechanical enforcement; a separate feature will land it.
- This feature **does not** wire the rules into a pre-commit hook. CI on PR via `product verify --platform` is the only enforcement point in this feature's scope.
- This feature **does not** change product-cli. The cross-cutting ADR + TC pattern is already supported; we are consumers.
- This feature **does not** add new Rust or Python dependencies. Every script runs on `bash`, `awk`, `wc`, `find`, and the Python 3 standard library.

## Out of scope

- Authoring the ADRs themselves — that lives in ADR-013 (the rule definitions) and ADR-014 (the framing of "rules live in the internal product-cli graph").
- IDE-side enforcement (rust-analyzer plugins, ruff rules tied to file size, etc.). Editor integration is a separate concern.
- Migrating any existing oversized files in the workspace. This feature only ships the enforcement; remediation of existing violations is left to the implementation session and may produce zero, one, or more incidental refactors as needed to land green.
- A Python equivalent of `scripts/checks/module-structure.sh`. Workers are small enough that explicit module-structure assertions are not yet earned; this can be added once workers grow more than one package.
- Custom clippy lints or `cargo-deny` configuration. Per ADR-013, enforcement is by shell scripts so that any contributor can audit the rules by reading them.

## Derivation

This feature is adapted from product-cli FT-031 ("Code Structure and Quality Standards"). The four rules and their thresholds are inherited; the implementation set is extended to cover Python workers, and the SDP boundary on `crates/oxi-events` is acknowledged as a structural constraint that single-responsibility comments must honour (see ADR-013 §"Rule scope and the oxi-events SDP boundary").
