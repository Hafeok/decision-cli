---
id: FT-137
title: 'decision-cli: Defect-scope guard — extend allowlist with build/docs/CI/VCS defaults and project-level config'
phase: 4
status: complete
depends-on: []
adrs:
- ADR-078
tests:
- TC-341
- TC-342
- TC-343
- TC-344
domains:
- api
domains-acknowledged:
  observability: FT-137 ships 4 TCs (TC-341 exit-criteria + TC-342/343/344 scenarios) satisfying ADR-072. ADR-072 spans api + observability; api is a primary domain. Observability concerns are covered by TC-342 (integration test asserts finalize success without ScopeViolation — surfaces live finalize behaviour) and TC-343 (regression test asserts ScopeViolation still fires on out-of-scope files — observes live error surface). Explicit acknowledgement per ADR-072 review gate.
---

## Description

The implementation slice for [ADR-078](ADR-078)'s decision to extend the defect-scope guard's allowlist beyond `.product/` and `.dec/`. Today the guard hardcodes those two prefixes via `is_system_path` in `crates/decision-cli/src/features/finalize/mod.rs:200`. After this slice the predicate covers build manifests, repo-level docs, CI/packaging configs, and VCS metadata by default, plus a project-level `[scope-guard].always-allowed` config override.

The slice is small and surgical: rename `is_system_path` → `is_always_allowed`, expand its match list, add a `[scope-guard]` config reader, wire it through `FinalizeInput`. Witnessed motivating failure: [FT-136](FT-136) cross-cutting drive blocked at iteration 1 because Cargo.toml + crate deletions sat outside the narrow allowlist.

One subcommand → one slice — this slice is structural rather than verb-shaped; no new CLI surface.

## Functional Specification

### Inputs

- The current `decision-cli` workspace at `main`.
- The current `crates/decision-cli/src/features/finalize/mod.rs` source (the guard in `mod.rs:143-178`, the predicate in `mod.rs:200-202`).
- The current `crates/decision-cli/src/core/config/` (or wherever `.product/config.toml` is parsed today — locate during Phase 2).

### Outputs

- `is_always_allowed` predicate in `finalize/mod.rs` covering four default categories plus a `Vec<String>` of project-configured extras.
- A `[scope-guard]` table reader in the config module, returning `always_allowed: Vec<String>`.
- `FinalizeInput` gains a `scope_guard_extras: Vec<String>` field.
- Unit tests on the predicate (positive and negative cases per category).
- Integration tests: defect-fix iteration touching `Cargo.toml` + a prior-set file succeeds; defect-fix touching a non-allowed code file still raises `ScopeViolation`.

### State

- Updated on-disk: `crates/decision-cli/src/features/finalize/mod.rs`, `crates/decision-cli/src/features/finalize/tests.rs`, the config module reading `.product/config.toml`, the caller in `features/implement/` that builds `FinalizeInput`.
- Preserved on-disk: `.product/`, `.dec/`, all workers, all other features.

### Behaviour

#### Phase 1 — Default allowlist expansion

1. Rename `fn is_system_path(path: &str) -> bool` to `fn is_always_allowed(path: &str, extras: &[String]) -> bool` for clarity. Callers updated.
2. Expand the predicate to match these categories:

   ```rust
   // Filename-only matches (basename equality, any depth).
   const ALLOWED_BASENAMES: &[&str] = &[
       "Cargo.toml", "Cargo.lock",
       "package.json", "package-lock.json",
       "pyproject.toml", "uv.lock",
       "pnpm-lock.yaml", "yarn.lock",
   ];

   // Prefix matches (path starts with).
   const ALLOWED_PREFIXES: &[&str] = &[
       ".product/", ".dec/",
       ".github/", ".cargo/",
   ];

   // Exact root-path matches.
   const ALLOWED_ROOT_FILES: &[&str] = &[
       "CLAUDE.md", "README.md", "CONTRIBUTING.md",
       "LICENSE", "LICENSE.md", "LICENSE.txt",
       "CODE_OF_CONDUCT.md", "CHANGELOG.md",
       ".gitignore", ".gitattributes",
       "dist-workspace.toml",
       "rust-toolchain.toml", "rust-toolchain",
   ];
   ```

3. The predicate body:
   ```rust
   fn is_always_allowed(path: &str, extras: &[String]) -> bool {
       if ALLOWED_PREFIXES.iter().any(|p| path.starts_with(p)) { return true; }
       if ALLOWED_ROOT_FILES.contains(&path) { return true; }
       if let Some(base) = std::path::Path::new(path).file_name().and_then(|s| s.to_str()) {
           if ALLOWED_BASENAMES.contains(&base) { return true; }
       }
       if matches_any_extra(path, extras) { return true; }
       false
   }
   ```

4. `matches_any_extra` supports `**` glob matching. Use the `glob` crate's `Pattern` if it's already in the tree, or hand-roll a simple `prefix-with-wildcard` matcher (the common case is `<dir>/**`).

#### Phase 2 — Project-level config

1. Extend the config module to read a `[scope-guard]` table from `.product/config.toml`:
   ```toml
   [scope-guard]
   always-allowed = [
       "scripts/checks/**",
       "deny.toml",
   ]
   ```
2. The reader returns `Vec<String>`; an absent section returns an empty vec (no error).
3. Malformed TOML at `[scope-guard]` → warn-log and proceed with defaults-only; do not panic.

#### Phase 3 — Wire through FinalizeInput

1. Add `scope_guard_extras: Vec<String>` to `FinalizeInput` in `crates/decision-cli/src/features/finalize/mod.rs`.
2. Callers in `features/implement/` (the `dec implement` handler chain) read `.product/config.toml` and populate `scope_guard_extras` before calling `finalize`.
3. Default for tests / non-production callers that don't populate it: empty vec (defaults-only).
4. The guard at `mod.rs:172` becomes:
   ```rust
   .filter(|(_, path)| !is_always_allowed(path, &input.scope_guard_extras) && !allowed.contains(path))
   ```

#### Phase 4 — Tests

1. **Unit tests** on `is_always_allowed` in `finalize/tests.rs`:
   - Positive: `Cargo.toml`, `crates/foo/Cargo.toml`, `Cargo.lock`, `pyproject.toml`, `.github/workflows/release.yml`, `.cargo/config.toml`, `CLAUDE.md`, `README.md`, `.gitignore`, `dist-workspace.toml`, `.product/features/FT-001.md`, `.dec/store/orchestration.nq`.
   - Negative (defaults only): `crates/decision-cli/src/main.rs`, `crates/decision-cli/tests/integration.rs`, `docs/architecture.md`.
   - Config-extras positive: `extras = ["scripts/checks/**"]` makes `scripts/checks/foo.sh` allowed.
   - Config-extras negative: same extras leave `unrelated/foo.sh` denied.
2. **Integration test** (TC-342): construct a `FinalizeInput` with `defect_scoped: true`, a prior `[FT-X]` commit touching `crates/foo/src/lib.rs`, and a dirty working tree modifying `Cargo.toml` + `crates/foo/src/lib.rs`. Assert finalize succeeds (no ScopeViolation).
3. **Regression test** (TC-343): same fixture but the dirty tree modifies `crates/foo/src/lib.rs` + `crates/bar/src/lib.rs` (the latter is not in the prior set and not allowlisted). Assert finalize returns `ScopeViolation` with `bar/src/lib.rs` in the paths.

### Invariants

- **Defaults need no config.** Absent `[scope-guard]` table → defaults-only, no error.
- **Extras are additive.** No syntax for removing a default; if a default is wrong, the code is patched.
- **Negative case still trips.** A feature-scoped file (e.g. `crates/decision-cli/src/foo/bar.rs`) not in the prior `[FT-XXX]` commit and not in the allowlist still raises `ScopeViolation`. The slice expands defaults, not weakens the guard.
- **Initial-implementation runs unchanged.** The `has_prior_implementation` bypass at `mod.rs:166` still applies; a feature with no prior code commits remains unrestricted.
- **No worker code changes.** Workers never see the allowlist; the guard runs orchestrator-side.

### Error handling

- **Malformed `[scope-guard]` section** → warn-log, proceed with defaults-only. (Drives must not halt on a config typo.)
- **Unrecognised glob syntax in `always-allowed`** → log a warning, treat the pattern as a literal prefix.
- **`.product/config.toml` missing entirely** → defaults-only, no error.

### Boundaries

- **In scope.** The four phases above; default allowlist expansion; `[scope-guard].always-allowed` config reader; FinalizeInput threading; unit + integration tests.
- **Out of scope.** Per-feature `scope-extra: [...]` frontmatter (deferred per ADR-078). Subtractive overrides. Spec-body-parsing approach. Changes to non-defect-scoped runs. CLI flag for ad-hoc overrides.

## Out of scope

- Per-feature spec-frontmatter overrides.
- Subtractive overrides.
- Spec-body file-path parsing.
- CLI flag for one-off allowlist additions.
- Documentation overhaul beyond inline rustdoc on the predicate.
- Backporting older commits' allowed sets.
