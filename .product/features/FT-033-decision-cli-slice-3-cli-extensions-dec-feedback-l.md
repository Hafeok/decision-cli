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
domains-acknowledged: {}
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
