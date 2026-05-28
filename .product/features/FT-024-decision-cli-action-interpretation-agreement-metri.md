---
id: FT-024
title: 'decision-cli: Action-interpretation agreement metric and dec metrics surface'
phase: 2
status: planned
depends-on:
- FT-020
- FT-021
adrs:
- ADR-018
- ADR-021
tests:
- TC-032
domains: []
domains-acknowledged: {}
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
