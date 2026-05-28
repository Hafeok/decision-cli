---
id: FT-018
title: 'decision-cli: Migrate to vertical-slice layout under SDP'
phase: 1
status: planned
depends-on: []
adrs:
- ADR-016
- ADR-001
- ADR-013
- ADR-014
tests:
- TC-020
domains: []
domains-acknowledged: {}
---

## Description

Land the vertical-slice layout codified in [ADR-016](ADR-016) on `crates/decision-cli/`. Today the crate is a flat row of sibling modules at `crates/decision-cli/src/{init,implement,events,health,session_inspect,scope,stream_writer,ontology,bundled,finalize,vocab}.rs`. After this feature, the crate is reorganised into two top-level subtrees — `core/` (stable primitives) and `features/` (vertical slices) — with `main.rs` shrunk to the thinnest possible wiring layer. Cross-feature imports become a compile error; the existing CLI behaviour is preserved bit-for-bit (every passing TC continues to pass).

This is a structural refactor, not a behavioural one. It exists to make slice 2 onward authorable without bleeding context across features and to make the SDP layering enforceable by the compiler rather than by code review.

## Functional Specification

### Inputs

- The current `crates/decision-cli/src/` tree (12 top-level modules + `main.rs` + `lib.rs`).
- ADR-016 (the layering contract).
- The existing test suite under `crates/decision-cli/tests/` — these tests are the regression net for behavioural equivalence.

### Outputs

- A reorganised crate source tree:

```
crates/decision-cli/src/
  lib.rs
  main.rs                # ≤ 250 lines; clap tree + dispatch only
  core/
    mod.rs               # pub use ontology::*; pub use vocab::*; …
    ontology.rs          # moved from src/ontology.rs (or src/ontology/)
    vocab.rs             # moved from src/vocab.rs
    bundled.rs           # moved from src/bundled.rs (or src/bundled/)
    scope.rs             # moved from src/scope.rs
    stream_writer.rs     # moved from src/stream_writer.rs
    store.rs             # NEW — extracts the `.dec/store/orchestration.nq`
                         # load / dump dance currently inlined in init,
                         # status, sparql, session_inspect, events.
    sparql.rs            # NEW — `term_iri_string`, `term_literal_string`,
                         # and the small SPARQL-result helpers currently
                         # duplicated across modules and main.rs.
  features/
    mod.rs               # `pub mod init;` etc.; NO cross-imports.
    init/mod.rs          # pub run, pub InitArgs, pub InitOutcome, pub InitError
    implement/mod.rs     # pub run, pub session_show, pub ImplementArgs,
                         # pub ImplementOutcome
    health/mod.rs
    events/mod.rs
    session_inspect/mod.rs
    finalize/mod.rs
```

- A new cross-cutting TC backed by `scripts/checks/vertical-slice-imports.sh` that fails CI on any feature-to-feature import.
- `lib.rs` re-export surface unchanged from the *outside* (so integration tests under `crates/decision-cli/tests/` keep compiling).
- `main.rs` shrunk: the `term_iri_string` / `term_literal_string` helpers and the inline SPARQL strings for `dec status` and `dec _sparql` move into the relevant feature modules (`features::init::status` and either `features::init` or a new `features::sparql` thin wrapper).

### State

- No persistent state changes. The orchestration store on disk (`.dec/store/orchestration.nq`) is untouched — its read/write code simply moves from `init`/`main` into `core::store`.

### Behaviour

1. Move modules into `core/`:
   - `ontology` (single-file or directory form, preserved as-is) → `core/ontology.rs` (or `core/ontology/`).
   - `vocab.rs` → `core/vocab.rs`.
   - `bundled.rs` (and its `bundled/value_actions/` submodule, if any) → `core/bundled.rs` (or `core/bundled/`).
   - `scope.rs` → `core/scope.rs`.
   - `stream_writer.rs` → `core/stream_writer.rs`.
2. Create `core/store.rs` and migrate the duplicated store-load dance.
3. Create `core/sparql.rs` and migrate `term_iri_string` / `term_literal_string` + thin result-row helpers.
4. Move each feature module into `features/<name>/mod.rs`. Where a feature currently has internal helpers in its own file (e.g. `implement.rs`'s helpers), split those into `pub(super)` submodules under `features/<name>/`.
5. Rewrite imports across all moved files:
   - `use crate::ontology::*` → `use crate::core::ontology::*` (or `use crate::core::*` for re-exported types).
   - `use crate::scope::*` → `use crate::core::scope::*`.
   - Cross-feature imports (none should exist today; verify) → fail the build.
6. Tighten `lib.rs`:
   ```rust
   pub mod core;
   pub mod features;
   pub use core::{ActiveScope, ScopeError, OntologyError, OntologyHandle,
                  ONTOLOGY_VERSION, StreamWriter};
   pub use features::finalize::{finalize_run, FinalizeError, FinalizeInput, FinalizeOutcome};
   pub use features::health::{check as health_check, HealthReport};
   pub use features::implement::{ImplementArgs, ImplementOutcome};
   ```
   The existing externally-used names stay re-exported so `tests/` and `main.rs` only need import-path edits (not API edits).
7. Shrink `main.rs`:
   - Remove the inline SPARQL strings for `run_status` and `run_sparql`; relocate them into `features::init::status` (a new `pub fn status(workdir, …) -> StatusReport`) and `features::init::sparql_query` (or keep `_sparql` as a thin `core::store::query_persisted` wrapper inside main).
   - Remove `term_iri_string` / `term_literal_string` from main.rs; they live in `core::sparql`.
   - Each command's `run_*` function becomes a 5–15 line shim that constructs the feature's args struct, calls `features::<feature>::run(...)`, and prints the result.
8. Add `scripts/checks/vertical-slice-imports.sh`:
   - Walks `crates/decision-cli/src/features/*/**/*.rs`.
   - Grep for `use crate::features::<sibling>` or `use super::super::<sibling>` patterns.
   - Exit 1 on the first violation with the file/line/import shown; exit 0 on a clean tree.
9. Author the cross-cutting TC for the check (`validates.adrs: [ADR-016]`, runner `bash`).
10. Run `cargo test --workspace` and `cargo clippy --workspace -- -D warnings` and confirm green before commit.

### Invariants

- **No cross-feature imports.** Statically verified by the new TC and dynamically by the compiler (sibling modules are unreachable through the module tree).
- **No behavioural change.** Every TC that passes today (TC-001..TC-019 inclusive of slice 1 + ADR-013/ADR-014 rules) continues to pass after migration. This is the regression gate.
- **`main.rs` line count drops.** `main.rs` ends below 250 lines. Even the looser ADR-013 §Rule 3 cap of 80 is the long-term aim, but reaching 80 may require feature-level refactors that are out of scope here.
- **`lib.rs` external surface is preserved.** Anyone importing `decision_cli::ImplementArgs` etc. keeps working.

### Error handling

- If a module move surfaces a hidden cross-feature dependency that can't be cleanly resolved by promoting to `core/`, stop the migration, record the discovery in a new ADR-016 amendment proposal, and pause this feature until the underlying coupling is named and resolved.
- The new `vertical-slice-imports.sh` script reports violations to stdout with file path and the offending `use` line; exits 1 so `product verify --platform` blocks the merge.

### Boundaries

- **In scope.** The Rust source layout of `crates/decision-cli/`, the new `core::store` + `core::sparql` modules, the `main.rs` shrink, the new check script + TC.
- **Out of scope.** Any change to `crates/oxi-events/`. Any change to the on-disk store format. Any behavioural change to existing commands. Splitting `features::implement/` further into multiple files beyond what is already factored out (a follow-up if file-length tips over).
- **Touched outside `crates/decision-cli/`**: only `scripts/checks/vertical-slice-imports.sh` and the new TC file.

## Out of scope

- Migrating `workers/code-writer/` into a slice layout (Python; different concerns).
- Adding new features. Slice 2 features (`dec dispatch`, `dec watch`, etc.) ride on top of this layout but are not part of this feature.
- Tightening `main.rs` to the ADR-013 80-line cap — that requires extracting feature-level output formatters and is tracked as future work.
- Splitting `core/` further (e.g. into `core::onto` and `core::store`) beyond the modules listed above.
- Renaming any externally re-exported symbol.
