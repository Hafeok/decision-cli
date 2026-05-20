---
id: FT-024
title: 'decision-cli: Action-interpretation agreement metric and dec metrics surface'
phase: 2
status: complete
depends-on:
- FT-020
- FT-021
adrs:
- ADR-018
- ADR-021
tests:
- TC-032
domains: []
domains-acknowledged:
  ADR-023: ADR-023 (feedback class controlled vocabulary) is implemented by FT-028; FT-024 produces no feedback artifacts.
  ADR-025: ADR-025 (blocking vs non-blocking feedback semantics) is implemented by FT-032; FT-024 has no feedback to gate.
  ADR-017: ADR-017 (action-interpretation pairing) is a Slice 2 structural requirement implemented by FT-021; FT-024 is out of scope for the pairing.
  ADR-004: ADR-004 (PROV-O) governs session/event lineage; FT-024 produces no new Session or event type and inherits lineage from the harness.
  ADR-013: ADR-013 (code structure standards) applies workspace-wide; FT-024's code conforms to cargo/clippy/rustfmt and the module-size convention. ADR-013 itself is owned by FT-014.
  ADR-001: ADR-001 governs the oxi-events crate boundary; FT-024 does not cross or alter that boundary.
  ADR-002: ADR-002 (graph-as-state) governs persistence semantics; FT-024 reads/writes via the GraphWriter chokepoint and does not introduce event-sourced state.
  ADR-027: ADR-027 (authority declarations in role catalog) is implemented by FT-030; FT-024 does not introduce or modify a role catalog entry.
  ADR-005: ADR-005 (value-stream scope) governs command-time scope; FT-024 runs inside an already-scoped command and does not introduce a new scope check.
  ADR-014: ADR-014 (fitness functions tracked as artifacts) is owned by FT-014/FT-015; FT-024 does not author or modify a fitness-function artifact.
  ADR-012: ADR-012 (per-stream working directory discovery) governs CLI entry; FT-024 runs after the working directory is resolved and does not re-discover it.
  ADR-016: ADR-016 (vertical-slice + compile-time SDP) is migrated by FT-018; FT-024's code is reorganised under that migration, not by this feature.
  ADR-022: ADR-022 (feedback as a first-class flow class) is a Slice 3 concern implemented by FT-026; FT-024 neither emits nor routes feedback.
  ADR-024: ADR-024 (feedback lifecycle state machine) is implemented by FT-027; FT-024 produces no feedback artifacts.
---

## Description

Compute and expose the four action-interpretation agreement metrics defined in [ADR-021](ADR-021): **agreement rate**, **amendment rate**, **rejection rate**, **false-success rate**. Surface them via `dec metrics agreement` (slice-2 CLI extension in [FT-025](FT-025)) and via a programmatic `core::metrics::agreement(...)` API.

This is the first fitness metric to land in the codebase. Phase C will broaden the surface; this feature lands the substrate.

## Functional Specification

### Inputs

- The orchestration store (post-init, populated by completed `DispatchGroup` artifacts from [FT-021](FT-021)).
- Optional window: `since`, `until` ISO-8601 timestamps.
- Optional filter: `role` (defaults to all action roles; in slice 2 there is only "implementer").

### Outputs

- A `core::metrics::AgreementReport` struct:
  ```rust
  pub struct AgreementReport {
      pub total_terminal_groups: u64,
      pub total_action_success: u64,
      pub approved: u64,
      pub amendment_required: u64,
      pub rejected: u64,
      pub agreement_rate: f64,
      pub amendment_rate: f64,
      pub rejection_rate: f64,
      pub false_success_rate: f64,
      pub window: Option<(DateTime<Utc>, DateTime<Utc>)>,
      pub role_filter: Option<String>,
  }
  ```
- A SPARQL query (parameterised by window + role) in `core::metrics::queries`.
- A pretty-print render for the CLI (5-row table).

### State

- Read-only. No mutations.

### Behaviour

1. Author the SPARQL query for the four counts (total `DispatchGroup` with terminal status, action-success subset, per-verdict counts). The query joins `DispatchGroup` → action `Session` → interpretation `Session` → `VerificationVerdict`.
2. Compute the rates from the counts in Rust (avoid SPARQL `xsd:double` division surprises).
3. Expose `core::metrics::agreement(store, window, role_filter) -> Result<AgreementReport, _>`.
4. Per the slice-level SDP convention, this lives in `core/` because Phase C will broaden the metric surface and other features (Phase B fitness functions) will consume it. No sibling feature reaches into this module's internals.
5. The CLI subcommand `dec metrics agreement` ([FT-025](FT-025)) constructs the args and prints the report.

### Invariants

- Rates are computed as `f64` with `NaN` substitution: when the denominator is zero (no qualifying dispatches in window), the rate is `0.0` and the report flags `total_terminal_groups = 0` so downstream consumers can distinguish "no data" from "data with zero rate."
- The query honors [ADR-005](ADR-005) — only dispatch groups in the active stream are counted.
- Adding a new verdict value ([ADR-018](ADR-018) amendment) requires extending the query and the report struct atomically.

### Error handling

- SPARQL execution failure → `MetricsError::Sparql { detail }`.
- Malformed window (start > end) → `MetricsError::InvalidWindow`.
- Unknown role filter → `MetricsError::UnknownRole { id }` (cross-checks against `core::role_catalog` from [FT-019](FT-019)).

### Boundaries

- **In scope.** SPARQL query, `core::metrics::agreement` API, report struct, pretty-print.
- **Out of scope.** CLI subcommand wiring ([FT-025](FT-025)). Continuous dashboard / streaming metrics (Phase C). Per-feature breakdown (Phase C). Time-series storage of historical metrics (Phase C). Threshold-driven release gates (Phase C).

## Out of scope

- Aggregating across multiple value streams (slice 2 is single-stream per [ADR-005](ADR-005)).
- Per-model metric breakdown (Phase B once model catalog is a graph artifact).
- Pattern detection / anomaly surfacing (Phase D meta-loop).
