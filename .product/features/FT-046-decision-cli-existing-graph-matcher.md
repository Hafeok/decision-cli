---
id: FT-046
title: 'decision-cli: existing-graph matcher'
phase: 2
status: complete
depends-on:
- FT-036
- FT-045
adrs:
- ADR-028
- ADR-030
tests:
- TC-070
- TC-071
- TC-072
domains: []
domains-acknowledged:
  ADR-002: ADR-002 (graph-as-state) governs persistence semantics; FT-046 is a read-only SPARQL primitive that performs no writes.
  ADR-027: ADR-027 (authority declarations in role catalog) is implemented by FT-030; FT-046 does not introduce or modify a role catalog entry.
  ADR-025: ADR-025 (blocking vs non-blocking feedback semantics) is implemented by FT-032; FT-046 has no feedback to gate.
  ADR-004: ADR-004 (PROV-O) governs session/event lineage; FT-046 produces no new Session or event type.
  ADR-014: ADR-014 (fitness functions tracked as artifacts) is owned by FT-014/FT-015; FT-046 does not author or modify a fitness-function artifact.
  ADR-016: ADR-016 (vertical-slice + compile-time SDP) is migrated by FT-018; FT-046's code is organised under that migration as core substrate.
  ADR-001: ADR-001 governs the oxi-events crate boundary; FT-046 does not cross or alter that boundary.
  ADR-021: ADR-021 (action-interpretation agreement metric) is a Slice 2 fitness function implemented by FT-024; FT-046 produces no action/interpretation pair.
  ADR-005: ADR-005 (value-stream scope) governs command-time scope; FT-046 runs inside an already-scoped command and does not introduce a new scope check.
  ADR-013: ADR-013 (code structure standards) applies workspace-wide; FT-046's code conforms to cargo/clippy/rustfmt and the module-size convention. ADR-013 itself is owned by FT-014.
  ADR-012: ADR-012 (per-stream working directory discovery) governs CLI entry; FT-046 has no CLI entry of its own and inherits the resolved working directory from its callers.
  ADR-022: ADR-022 (feedback as a first-class flow class) is a Slice 3 concern implemented by FT-026; FT-046 neither emits nor routes feedback.
  ADR-024: ADR-024 (feedback lifecycle state machine) is implemented by FT-027; FT-046 produces no feedback artifacts.
  ADR-023: ADR-023 (feedback class controlled vocabulary) is implemented by FT-028; FT-046 produces no feedback artifacts.
  ADR-017: ADR-017 (action-interpretation pairing) is a Slice 2 structural requirement implemented by FT-021; FT-046 is out of scope for the pairing.
  ADR-018: ADR-018 (VerificationVerdict schema) is a Slice 2 artifact implemented by FT-020; FT-046 neither emits nor consumes verdicts.
---

## Description

The existing-graph matcher — given a feature and a target environment, return the *best matching* set of pre-existing `dec:VerificationGraph` artifacts (i.e. the smallest set whose union covers all of the feature's TCs in that environment). Pure substrate built on top of [FT-045](FT-045)'s coverage primitive. Used by [ADR-030](ADR-030)'s verify-graph-author to decide whether to propose a `Match` or a `New` graph.

This feature exists because match-or-generate is a **deterministic Rust decision**, not an LLM judgement. The author worker sees the matcher's output as part of its bundle but does not run the matcher itself.

One subcommand → one slice — again no subcommand. Pure primitive with a focused `pub` surface.

## Functional Specification

### Inputs

- A feature id.
- A target environment id (matching is *per environment*; a graph in `ENV-001 (ephemeral-cli)` is not a match for a query that targets `ENV-002 (dev-deployment)`).
- The orchestration store handle.

### Outputs

- `MatchReport`:
  ```rust
  pub struct MatchReport {
      pub feature: FeatureId,
      pub environment: EnvId,
      pub kind: MatchKind,
      pub graphs: Vec<GraphSummary>,     // ordered: complete-match graphs first, then partial
      pub covered_by_match: Vec<TcId>,   // TCs covered by the returned set
      pub residual_uncovered: Vec<TcId>, // TCs the matcher could not cover in this env
  }
  pub enum MatchKind {
      CompleteSingle,      // one graph covers everything
      CompleteMultiple,    // a small set covers everything (graphs is the cover)
      Partial,             // matcher found something but residual is non-empty
      None,                // no graph in this env touches any of the feature's TCs
  }
  pub struct GraphSummary {
      pub id: GraphId,
      pub verifies: ArtifactRef,
      pub covers: Vec<TcId>,  // subset of all_tcs covered by this graph alone
  }
  ```
- `fn best_matching_graphs(feature: FeatureId, env: EnvId, store: &Store) -> Result<MatchReport>`

### State

- None. Read-only.

### Behaviour

1. Resolve the feature's TCs.
2. Enumerate every `dec:VerificationGraph` whose `dec:environment` equals the requested env (SPARQL query, parameterised on `env`).
3. For each candidate, call [FT-045](FT-045)'s `feature_covered_by` to compute its individual coverage subset.
4. Drop candidates whose coverage subset is empty — they touch none of this feature's TCs.
5. If any single candidate covers all TCs → `CompleteSingle`, return that graph alone.
6. Otherwise compute a **minimum cover** greedy over the remaining candidates:
   - At each step, pick the candidate covering the most still-uncovered TCs; ties broken by ascending `VG-NNN` numeric suffix.
   - Stop when residual is empty (→ `CompleteMultiple`) or when no candidate can extend the cover (→ `Partial`).
7. If no candidate remained after step 4 → `None`.
8. Return the `MatchReport`.

### Invariants

- Side-effect-free.
- Per-env scoping is strict — a graph in a different env is never considered, even if it covers all TCs.
- Greedy minimum cover is **deterministic** under the tiebreak rule (lowest numeric suffix). Two runs over the same store return the same `graphs` list in the same order.
- `MatchKind::None` ≠ `MatchKind::Partial` ≠ failure — all three are valid outcomes; the caller (worker bundle assembler, slice 3 executor) decides what to do.
- `residual_uncovered` and `covered_by_match` partition `all_tcs` for `Complete*` and `Partial`; for `None`, `residual_uncovered = all_tcs`, `covered_by_match = []`.

### Error handling

- Unknown feature → `Error::ArtifactNotFound { kind: "Feature", id }`.
- Unknown env → `Error::ArtifactNotFound { kind: "VerificationEnvironment", id }`.
- Store unreachable → `Error::StoreUnreachable`.

### Boundaries

- **In scope.** Per-env matching, greedy minimum cover, deterministic tiebreak, the `MatchReport`/`MatchKind`/`GraphSummary` types, integration tests over an in-memory store with seeded graphs.
- **Out of scope.** Cross-env composite matches (slice 3+, when execution lands and "the right env" becomes meaningful). Weighted preference (e.g. preferring graphs with newer timestamps). LLM-based fuzzy matching (intentionally never — the matcher is deterministic).

## Out of scope

- Cross-environment matching.
- Author-time graph recommendation (this is the *matcher*; recommendation is [FT-049](FT-049)'s remit).
- Persisting match reports.
- Time-windowed / activity-weighted matching.
