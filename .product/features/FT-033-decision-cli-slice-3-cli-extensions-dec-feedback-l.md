---
id: FT-033
title: 'decision-cli: Slice 3 CLI extensions (dec feedback list/show/close/route)'
phase: 2
status: planned
depends-on:
- FT-012
- FT-026
- FT-027
- FT-029
- FT-032
adrs:
- ADR-011
- ADR-022
tests:
- TC-039
domains: []
domains-acknowledged:
  ADR-014: ADR-014 (fitness functions tracked as artifacts) is owned by FT-014/FT-015; FT-033 does not author or modify a fitness-function artifact.
  ADR-013: ADR-013 (code structure standards) applies workspace-wide; FT-033's code conforms to cargo/clippy/rustfmt and the module-size convention. ADR-013 itself is owned by FT-014.
  ADR-004: ADR-004 (PROV-O) governs session/event lineage; FT-033 produces no new Session or event type and inherits lineage from the harness.
  ADR-012: ADR-012 (per-stream working directory discovery) governs CLI entry; FT-033 runs after the working directory is resolved and does not re-discover it.
  ADR-025: ADR-025 (blocking vs non-blocking feedback semantics) is implemented by FT-032; FT-033 has no feedback to gate.
  ADR-017: ADR-017 (action-interpretation pairing) is a Slice 2 structural requirement implemented by FT-021; FT-033 is out of scope for the pairing.
  ADR-002: ADR-002 (graph-as-state) governs persistence semantics; FT-033 reads/writes via the GraphWriter chokepoint and does not introduce event-sourced state.
  ADR-024: ADR-024 (feedback lifecycle state machine) is implemented by FT-027; FT-033 produces no feedback artifacts.
  ADR-001: ADR-001 governs the oxi-events crate boundary; FT-033 does not cross or alter that boundary.
  ADR-027: ADR-027 (authority declarations in role catalog) is implemented by FT-030; FT-033 does not introduce or modify a role catalog entry.
  ADR-018: ADR-018 (VerificationVerdict schema) is a Slice 2 artifact implemented by FT-020; FT-033 neither emits nor consumes verdicts.
  ADR-023: ADR-023 (feedback class controlled vocabulary) is implemented by FT-028; FT-033 produces no feedback artifacts.
  ADR-016: ADR-016 (vertical-slice + compile-time SDP) is migrated by FT-018; FT-033's code is reorganised under that migration, not by this feature.
  ADR-021: ADR-021 (action-interpretation agreement metric) is a Slice 2 fitness function implemented by FT-024; FT-033 produces no action/interpretation pair.
  ADR-005: ADR-005 (value-stream scope) governs command-time scope; FT-033 runs inside an already-scoped command and does not introduce a new scope check.
---

## Description

The slice-3 CLI extensions on `dec`. Four additions: `dec feedback list`, `dec feedback show`, `dec feedback close`, `dec feedback route`. Together with the slice-2 CLI from [FT-025](FT-025), they give operators (and the Phase A human-as-spec-author role) a complete inspection and resolution surface for feedback.

## Functional Specification

### Inputs

- The feedback schema and read API from [FT-026](FT-026).
- The lifecycle state machine from [FT-027](FT-027).
- The routing table from [FT-029](FT-029).
- The existing slice-1 / slice-2 CLI ([FT-012](FT-012), [FT-025](FT-025)).

### Outputs

- `dec feedback list [--state STATE] [--class CLASS] [--target ROLE]`:
  - Default: open feedback (non-terminal states).
  - Columns: IRI, class, state, target role, source feature/session, evidence (truncated), routed-at.
  - Exit 0; exit 2 on usage error.
- `dec feedback show <iri>`:
  - Full feedback record: every predicate, formatted as a key-value table.
  - Lifecycle history (read from named-graph history per [ADR-002](ADR-002)).
  - Addressing artifact (if `addressed` or `closed`) — rendered as a clickable IRI / file path.
- `dec feedback close <iri> --addressing <artifact-iri>`:
  - Transition `addressed → closed`.
  - Validates the addressing artifact exists and is in the same stream.
  - Validates the addressing artifact's type is in the routing-table's `addressing-roles` allowlist for the feedback's class (e.g. a `gap` requires a feature_spec amendment or a new ADR, not a `CodeChange`).
  - Calls [FT-032](FT-032)'s `resume_check` after a successful transition so any blocked dispatches resume.
- `dec feedback route <iri> --to <role-id>`:
  - Manual routing override after initial routing (also usable to re-route a `rejected` feedback by transitioning through a new feedback emission — Phase B; for Phase A, the override only applies pre-routing).
  - Records `dec:routingOverride` and the operator's identity (read from environment / git config).
- `dec feedback receive <iri>`:
  - Manual `routed → received` transition. Phase A path for human-as-target-role: the human acks they're working on it.

### State

- Read commands: no mutations.
- Write commands: each command performs exactly one state transition through [FT-027](FT-027)'s validator.

### Behaviour

1. Per the slice-level SDP, each subcommand lives in its own feature directory:
   - `features/ft_033a_feedback_list/`
   - `features/ft_033b_feedback_show/`
   - `features/ft_033c_feedback_close/`
   - `features/ft_033d_feedback_route/`
   - `features/ft_033e_feedback_receive/`
2. Implement each as a thin shim: parse args → call the corresponding `core::feedback::*` API → render output.
3. `dec feedback close` is the most complex: it validates addressing artifact eligibility per the routing table and then triggers resume on any paused dispatches.
4. Output format: aligned tabular text for `list`, key-value for `show`. JSON output via `--format json` on every subcommand (matches slice-1 conventions where applicable).

### Invariants

- All transitions go through `StreamWriter`; the CLI never writes raw RDF.
- Read commands are scoped to the active stream ([ADR-012](ADR-012)).
- `close` is refused if the feedback is not in `addressed` state (lifecycle invariant from [FT-027](FT-027)).
- `close` is refused if the addressing artifact is not in the routing-table's allowed type set.

### Error handling

- Unknown feedback IRI → `Error::ArtifactNotFound`, exit 1.
- Lifecycle transition invalid for current state → exits 1 with the validator's error message.
- Addressing artifact wrong type for the class → `Error::IneligibleAddressingArtifact { class, artifact_type, allowed_types }`, exit 1.
- Empty store / no feedback in window → `dec feedback list` prints the empty list and exits 0.
- `--format json` errors are still printed as JSON on stderr.

### Boundaries

- **In scope.** The five CLI subcommands, output formatting, plumbing into the args parser.
- **Out of scope.** Bulk operations (closing N feedback at once) — Phase B. Feedback authoring via CLI (workers emit; humans only close/route in Phase A). Live event tailing of feedback transitions — covered by the existing `dec events tail` from [FT-012](FT-012) (the events emitted by [FT-029](FT-029) flow through it automatically).

## Out of scope

- A TUI for feedback triage (Phase C+).
- Cross-stream feedback aggregation (single-stream Phase A).
- Feedback authoring through `product-cli` (not the right boundary — feedback is graph-resident).
