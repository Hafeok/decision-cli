---
id: FT-025
title: 'decision-cli: Slice 2 CLI extensions (dec verify, paired session display)'
phase: 2
status: planned
depends-on:
- FT-012
- FT-021
- FT-022
- FT-024
adrs:
- ADR-011
- ADR-017
- ADR-021
tests:
- TC-031
domains: []
domains-acknowledged:
  ADR-012: ADR-012 (per-stream working directory discovery) governs CLI entry; FT-025 runs after the working directory is resolved and does not re-discover it.
  ADR-027: ADR-027 (authority declarations in role catalog) is implemented by FT-030; FT-025 does not introduce or modify a role catalog entry.
  ADR-004: ADR-004 (PROV-O) governs session/event lineage; FT-025 produces no new Session or event type and inherits lineage from the harness.
  ADR-005: ADR-005 (value-stream scope) governs command-time scope; FT-025 runs inside an already-scoped command and does not introduce a new scope check.
  ADR-013: ADR-013 (code structure standards) applies workspace-wide; FT-025's code conforms to cargo/clippy/rustfmt and the module-size convention. ADR-013 itself is owned by FT-014.
  ADR-025: ADR-025 (blocking vs non-blocking feedback semantics) is implemented by FT-032; FT-025 has no feedback to gate.
  ADR-001: ADR-001 governs the oxi-events crate boundary; FT-025 does not cross or alter that boundary.
  ADR-023: ADR-023 (feedback class controlled vocabulary) is implemented by FT-028; FT-025 produces no feedback artifacts.
  ADR-002: ADR-002 (graph-as-state) governs persistence semantics; FT-025 reads/writes via the GraphWriter chokepoint and does not introduce event-sourced state.
  ADR-018: ADR-018 (VerificationVerdict schema) is a Slice 2 artifact implemented by FT-020; FT-025 neither emits nor consumes verdicts.
  ADR-014: ADR-014 (fitness functions tracked as artifacts) is owned by FT-014/FT-015; FT-025 does not author or modify a fitness-function artifact.
  ADR-024: ADR-024 (feedback lifecycle state machine) is implemented by FT-027; FT-025 produces no feedback artifacts.
  ADR-022: ADR-022 (feedback as a first-class flow class) is a Slice 3 concern implemented by FT-026; FT-025 neither emits nor routes feedback.
  ADR-016: ADR-016 (vertical-slice + compile-time SDP) is migrated by FT-018; FT-025's code is reorganised under that migration, not by this feature.
---

## Description

The slice-2 CLI extensions on `dec`. Three additions: a manual verifier trigger, paired-session display in `dec session show`, and the `dec metrics agreement` surface for [FT-024](FT-024). Together they make slice 2 inspectable from the operator's perspective.

## Functional Specification

### Inputs

- The existing slice-1 CLI ([FT-012](FT-012)).
- The `DispatchGroup` lifecycle ([FT-021](FT-021)).
- The verdict schema ([FT-020](FT-020)).
- The metrics API ([FT-024](FT-024)).

### Outputs

- New subcommand `dec verify <session-or-group-iri>`:
  - If the argument is an action session, find its `DispatchGroup` and re-emit the verifier-dispatch event (matches the recovery path mentioned in [FT-022](FT-022) error handling).
  - If the argument is a `DispatchGroup`, same behaviour.
  - Exit 0 on event published; exit 1 with structured error otherwise.
- Extended `dec session show <iri>`:
  - When the session is part of a `DispatchGroup`, display the group's status, the paired session (action or interpretation), and the verdict (if any) with its rationale.
  - Format mirrors `dec status`'s output shape.
- Extended `dec session log <iri>`:
  - Walk PROV-O across both sessions of the group, producing a single chronological log.
- New subcommand `dec metrics agreement [--since ISO] [--until ISO] [--role ROLE]`:
  - Calls [FT-024](FT-024)'s `core::metrics::agreement`.
  - Prints the 5-row table; exit 0 on success.

### State

- Read-only commands except `dec verify`, which writes one event through the outbox (idempotent — if a verifier-dispatch event already exists for the group, `dec verify` emits an additional one which the worker's idempotency check should absorb).

### Behaviour

1. Following the slice-level SDP convention codified in `CLAUDE.md`, each subcommand lives in its own `features/ft_NNN_*/` directory once FT-018's migration lands. For Phase A authoring, this feature_spec covers the *behavior*; the directory mapping is:
   - `features/ft_025a_dec_verify/`
   - `features/ft_025b_session_paired_display/` (extends the existing `features/ft_012*_session_inspect/`)
   - `features/ft_025c_dec_metrics/`
2. `dec verify <iri>`:
   - Parse argument; classify as Session vs. DispatchGroup via SPARQL `ASK`.
   - Re-emit the verifier-dispatch event through [FT-022](FT-022)'s delivery handler.
   - Print the emitted event's seq number; exit 0.
3. `dec session show` extension:
   - After existing slice-1 output, add a "Paired:" section if the session belongs to a `DispatchGroup`.
   - Verdict block: verdict, rationale (wrapped), violates list, amendment guidance.
4. `dec metrics agreement`:
   - Construct an `AgreementReport` request from CLI args.
   - Pretty-print:
     ```
     Action-Interpretation Agreement
     -------------------------------
     Window:                      2026-04-01T00:00:00Z .. 2026-05-19T13:00:00Z
     Role:                        implementer
     Total terminal dispatches:   34
     Action-success dispatches:   30
     Approved:                    24  (agreement rate 80.0%)
     Amendment-required:           5  (amendment rate 14.7%)
     Rejected:                     1  (rejection rate 2.9%)
     False-success rate:          20.0%
     ```

### Invariants

- The CLI never writes a `VerificationVerdict` directly; only the verifier worker does.
- `dec verify` does not bypass `StreamWriter` — it goes through the same delivery handler the orchestrator uses.
- All new subcommands honor [ADR-012](ADR-012) (per-stream working directories).

### Error handling

- Unknown IRI → `Error::ArtifactNotFound { iri }`, exit 1.
- IRI in a different stream than the active one → `Error::OutOfStream { iri, stream }`, exit 1.
- Metrics window with `since > until` → `Error::InvalidWindow`, exit 2 (usage error).
- Empty store (no dispatch groups) → `dec metrics agreement` prints the empty report and exits 0 (informational, not an error).

### Boundaries

- **In scope.** The three CLI behaviours, their plumbing into the existing args parser, the output formatting.
- **Out of scope.** Anything beyond display + manual trigger. Modifying verdicts manually (rejected as a design choice). Continuous dashboards (Phase C).

## Out of scope

- `dec drive` and `dec dispatch` surfaces (later slices per slice-1 bounds §9).
- A TUI for live dispatch monitoring (Phase C+).
- Multi-stream aggregation (single-stream Phase A).
