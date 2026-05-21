---
id: FT-047
title: 'decision-cli: chain-integrity dispatch gate'
phase: 2
status: planned
depends-on:
- FT-035
- FT-036
- FT-045
adrs:
- ADR-028
- ADR-031
tests:
- TC-073
- TC-074
- TC-075
domains: []
domains-acknowledged:
  ADR-001: ADR-001 governs the oxi-events crate boundary; FT-047 does not cross or alter that boundary.
  ADR-002: ADR-002 (graph-as-state) governs persistence semantics; FT-047 introduces the dec:CoverageWaiver artifact written via the StreamWriter chokepoint and does not introduce event-sourced state.
  ADR-004: ADR-004 (PROV-O) governs session/event lineage; FT-047 records waivers in the dispatch session's PROV-O chain via prov:used on the dispatch activity.
  ADR-005: ADR-005 (value-stream scope) governs command-time scope; FT-047 runs inside an already-scoped command and does not introduce a new scope check.
  ADR-012: ADR-012 (per-stream working directory discovery) governs CLI entry; FT-047 runs after the working directory is resolved and does not re-discover it.
  ADR-013: ADR-013 (code structure standards) applies workspace-wide; FT-047's code conforms to cargo/clippy/rustfmt and the module-size convention. ADR-013 itself is owned by FT-014.
  ADR-014: ADR-014 (fitness functions tracked as artifacts) is owned by FT-014/FT-015; FT-047 does not author the waiver-rate fitness function itself (slice 3+); it produces the waiver artifacts the future fitness function will measure.
  ADR-016: ADR-016 (vertical-slice + compile-time SDP) is migrated by FT-018; FT-047's gate lives in core::harness and its waiver writer is core substrate.
  ADR-017: ADR-017 (action-interpretation pairing) is a Slice 2 structural requirement implemented by FT-021; FT-047 enforces preconditions on dispatch but does not itself produce a paired action/interpretation.
  ADR-018: ADR-018 (VerificationVerdict schema) is a Slice 2 artifact implemented by FT-020; FT-047 neither emits nor consumes verdicts.
  ADR-021: ADR-021 (action-interpretation agreement metric) is a Slice 2 fitness function implemented by FT-024; FT-047 produces no action/interpretation pair.
  ADR-022: ADR-022 (feedback as a first-class flow class) is a Slice 3 concern implemented by FT-026; FT-047 surfaces a structured Error::ChainIntegrity but does not route a feedback artifact.
  ADR-023: ADR-023 (feedback class controlled vocabulary) is implemented by FT-028; FT-047 produces no feedback artifacts.
  ADR-024: ADR-024 (feedback lifecycle state machine) is implemented by FT-027; FT-047 produces no feedback artifacts.
  ADR-025: ADR-025 (blocking vs non-blocking feedback semantics) is implemented by FT-032; FT-047 has no feedback to gate (the gate it owns is on coverage, not feedback).
  ADR-027: ADR-027 (authority declarations in role catalog) is implemented by FT-030; FT-047 does not introduce or modify a role catalog entry.
---

## Description

The chain-integrity dispatch gate — implements the invariant defined in [ADR-031](ADR-031). The harness refuses to dispatch any worker that targets a `feature` artifact when the feature's TCs do not have full coverage by at least one `dec:VerificationGraph`, *unless* the caller supplies a `--waive-coverage <reason>` flag (CLI) / `accept_waiver: { reason }` field (MCP), in which case a `dec:CoverageWaiver` artifact is persisted as part of the dispatch.

This feature is **substrate**, not a verb. It modifies the existing dispatch path in `core::harness::dispatch` and introduces a new `dec:CoverageWaiver` artifact type with its own SHACL shape and on-disk Turtle file. The dispatch verbs themselves (`dec implement`, future `dec drive`, etc.) gain only a flag wiring; the gate logic lives in one place.

One subcommand → one slice — this slice covers exactly the gate. The waiver list / show / revoke verbs are slice 3+ concerns; for slice 2.6, waivers are written and readable but not separately surfaced.

## Functional Specification

### Inputs

- A `DispatchRequest { worker_role, target: ArtifactRef, waiver: Option<WaiverIntent> }` (the existing dispatch input, extended with the optional waiver field).
- The orchestration store handle.
- [FT-045](FT-045)'s coverage primitive.

### Outputs

- New `dec:CoverageWaiver` SHACL shape and Rust type:
  ```rust
  pub struct CoverageWaiver {
      pub id: WaiverId,                       // CW-NNN
      pub waiver_for: FeatureId,
      pub reason: String,                     // min 16 chars
      pub attributed_to: AgentId,             // PROV-O actor
      pub created: DateTime<Utc>,
      pub uncovered_at_waive: Vec<TcId>,      // snapshot of what was uncovered
  }
  ```
- On-disk path: `.dec/verify/waivers/CW-NNN.ttl`.
- New error variant: `Error::ChainIntegrity { feature: FeatureId, uncovered_tcs: Vec<TcId> }`.
- Gate wired into `core::harness::dispatch::pre_dispatch_checks` — runs *after* artifact resolution and *before* the worker invocation.
- CLI flag: `--waive-coverage <reason>` on every dispatch verb that takes a feature (slice 2.6: `dec implement`; the same flag is inherited by future verbs through a shared clap derive struct).
- MCP field: `accept_waiver: { reason: string }` on the equivalent MCP tools (slice 2.6: `dec_implement` if a tool exists; the wiring lives next to the CLI flag via the [ADR-029](ADR-029) single-handler discipline).

### State

- One new artifact type (`CoverageWaiver`) — SHACL shape, ontology IRI, on-disk path under `.dec/verify/waivers/`, named-graph projection.
- The dispatch handler now refuses by default for uncovered features.
- Each waiver written becomes part of the session's PROV-O chain via `prov:used <waiver-iri>` on the dispatch activity.

### Behaviour

1. `pre_dispatch_checks` resolves the target artifact. If it is not a feature, the gate short-circuits as "not applicable" (the slice 2.6 gate scope is feature-targeted dispatch only).
2. Otherwise, call [FT-045](FT-045)'s `feature_coverage(feature, None, &store)`.
3. If `coverage.uncovered.is_empty()` → gate passes.
4. If `coverage.uncovered` is non-empty and no waiver intent is present → return `Error::ChainIntegrity { feature, uncovered_tcs }`. The CLI converts this to exit 1 with a structured message that:
   - Names the uncovered TCs.
   - Suggests the next action: `dec verify graph generate <feature> --environment <env>` ([FT-049](FT-049)) for each known env.
   - Suggests the escape hatch: `--waive-coverage "<reason>"` (with the constraint that reason must be ≥ 16 chars).
5. If `coverage.uncovered` is non-empty *and* a waiver intent is present → validate the reason (≥ 16 chars, non-whitespace-only), mint `CW-NNN`, build the `CoverageWaiver` struct (capturing the snapshot of `uncovered` at this moment), persist via the StreamWriter chokepoint (SHACL re-validates), write the on-disk `.ttl`, and let the dispatch proceed. The waiver IRI is recorded on the dispatch session's PROV-O chain.
6. If the dispatch later fails for any reason, the waiver is **not** rolled back — it stays as a record that this dispatch was attempted with coverage gap, regardless of outcome.

### Invariants

- The gate runs on **every** feature-targeted dispatch, including replays and reruns. No bypass.
- Waiver writes go through the same `StreamWriter` chokepoint as all other artifacts ([ADR-002](ADR-002), [ADR-029](ADR-029)). No direct file-write paths.
- A waiver's `uncovered_at_waive` snapshot is captured **at gate-firing time**, not at session-end. The reader's mental model is "the waiver attests to *this* coverage gap as it existed when the dispatch began."
- The error message for `ChainIntegrity` must include actionable remediation (graph generate, waiver flag, list of uncovered TCs). A bad error here is the single worst outcome of [ADR-031](ADR-031); the message itself is part of the feature contract.
- The gate is **role-agnostic**: it does not care whether the dispatched worker is the implementer, a doc-writer, or a refactor role. The invariant is on the *target*, not the *worker*.
- The gate does **not** check environment-coverage match — that's slice 3+.

### Error handling

- `Error::ChainIntegrity { feature, uncovered_tcs }` — uncovered TCs and no waiver intent; exit 1.
- Waiver reason too short or whitespace-only → `Error::InvalidArgument { field: "waiver.reason", detail: "must be at least 16 non-whitespace characters" }`; exit 2.
- Waiver write fails (SHACL or I/O) → `Error::SchemaViolation` / `Error::Io`; exit 1; dispatch refused.
- Store unreachable → `Error::StoreUnreachable`; exit 1.

### Boundaries

- **In scope.** `Error::ChainIntegrity`; the gate function; the `--waive-coverage` flag + MCP equivalent; the `dec:CoverageWaiver` artifact + SHACL + IRI + on-disk path; PROV-O wiring of waiver into the dispatch activity; updates to `dec implement` to surface the gate's error well.
- **Out of scope.** Listing / showing / revoking waivers (slice 3+). Waiver-rate fitness function ([ADR-031](ADR-031) §"fitness-function corollary"; slice 3+). Per-environment coverage tightening (slice 3+). Auto-running the author worker as a remedy (that would couple the gate to the worker; the gate's job is to refuse, not to remediate).

## Out of scope

- `dec verify waivers list / show / revoke` (slice 3+).
- Auto-running [FT-048](FT-048)'s worker when the gate fires (couples gate to remedy).
- Per-environment coverage check.
- Waiver expiry / TTL.
- Waiver-rate fitness function.
- Suppressing the gate via environment variable or config flag — only the artifact-emitting flag is allowed.
