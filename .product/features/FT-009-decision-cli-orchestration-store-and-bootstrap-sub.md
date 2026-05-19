---
id: FT-009
title: 'decision-cli: Orchestration store and bootstrap subscriptions'
phase: 1
status: complete
depends-on:
- FT-001
- FT-002
adrs:
- ADR-002
- ADR-003
- ADR-005
- ADR-012
- ADR-001
- ADR-004
- ADR-008
tests:
- TC-001
- TC-002
- TC-015
- TC-019
domains: []
domains-acknowledged:
  ADR-025: ADR-025 (blocking vs non-blocking feedback semantics) is implemented by FT-032; FT-009 has no feedback to gate.
  ADR-014: FT-009 ships orchestration plumbing (store + bootstrap subscriptions); it does not author cross-cutting rules. Compliance is verified by the existing graph-check / verify pipeline.
  ADR-023: ADR-023 (feedback class controlled vocabulary) is implemented by FT-028; FT-009 produces no feedback artifacts.
  ADR-017: ADR-017 (action-interpretation pairing) is a Slice 2 structural requirement implemented by FT-021; FT-009 is out of scope for the pairing.
  ADR-022: ADR-022 (feedback as a first-class flow class) is a Slice 3 concern implemented by FT-026; FT-009 neither emits nor routes feedback.
  ADR-021: ADR-021 (action-interpretation agreement metric) is a Slice 2 fitness function implemented by FT-024; FT-009 produces no action/interpretation pair.
  ADR-013: FT-009 seeds bootstrap subscriptions in init/mod.rs which is already 914 lines (pre-existing ADR-013 violation owned by FT-014). The seeding logic is added as a single bounded helper at the end of the module; full remediation is FT-014.
  ADR-024: ADR-024 (feedback lifecycle state machine) is implemented by FT-027; FT-009 produces no feedback artifacts.
  ADR-027: ADR-027 (authority declarations in role catalog) is implemented by FT-030; FT-009 does not introduce or modify a role catalog entry.
  ADR-018: ADR-018 (VerificationVerdict schema) is a Slice 2 artifact implemented by FT-020; FT-009 neither emits nor consumes verdicts.
  ADR-016: ADR-016 (vertical-slice + compile-time SDP) is migrated by FT-018; FT-009's code is reorganised under that migration, not by this feature.
---

## Description

Each decision-cli value stream lives in its own working directory per **ADR-012 (Per-stream working directories)** with `.dec/config.toml` and an Oxigraph store at `.dec/store/`. This feature owns directory layout, store open/create, the v0 seed subscriptions per **ADR-003** bootstrapped on first startup, and the git-style discovery walk (ADR-012).

See `decision-cli-slice-1-bounds.md` §3.5, §5.3, §6.1.

## Functional Specification

### Inputs

- A working directory (defaulted to CWD, overridable for tests).
- A first-run flag (set by `dec init`, FT-008).

### Outputs

- An `OrchestrationStore` handle: Oxigraph store + a `GraphWriter` (FT-001) configured against it.
- The set of v0 seed subscriptions persisted in the store (ADR-003).

### State

- On-disk layout: `.dec/store/` (Oxigraph), `.dec/config.toml` (minimal in slice 1), ontology snapshot recorded at init.
- Working-directory resolution cache for process lifetime.

### Behaviour

1. On open, walk up from CWD to find the nearest `.dec/` (ADR-012); if none and not first-run, error.
2. Open the Oxigraph store at `.dec/store/`.
3. Construct the `GraphWriter` against it (FT-001).
4. If first-run, persist the v0 seed subscriptions: "dispatch available for code-writer," "code-writer dispatch completed" (ADR-003 — written as graph artifacts).
5. Return the handle.

### Invariants

- A repository has at most one orchestration store; nested `.dec/` is an error (ADR-012).
- After first-run, seed subscriptions are present and re-evaluable.
- The store handle is the unique mutation chokepoint for the process lifetime (ADR-002, FT-001).

### Error handling

- `StoreError::NotInitialized` — no `.dec/` found and not first-run; hints `dec init`.
- `StoreError::Open(_)` — corrupt store; no recovery attempted.
- `StoreError::NestedRepo { outer, inner }` — nested `.dec/` discovery.

### Boundaries

- Does NOT validate ValueStream definitions (FT-008).
- Does NOT enforce command-time scope (FT-010).
- Does NOT implement the CLI surface (FT-012).

## Out of scope

- Cross-machine store sync / replication.
- Store backup / restore commands.
- Multi-stream operation in a single process.

## Build-environment prerequisite

Implementing on-disk persistence requires the `rocksdb` feature on `oxigraph`, which the slice 1 scaffold disabled to keep the workspace building without system deps. Before opening a `Store::open(path)`:

1. Install system deps: `sudo apt install libclang-dev cmake` (Ubuntu/Debian) or equivalent for your distro.
2. In the workspace root `Cargo.toml`, change the `oxigraph` line back to default features:
   ```toml
   oxigraph = "0.4"   # was: { version = "0.4", default-features = false }
   ```
3. Re-run `cargo check --workspace` to confirm the build still passes.

The in-memory `Store::new()` API is available without the feature flag and is sufficient for unit tests that don't need persistence. The slice 1 scaffold's Cargo.toml comment names this same step.
