---
id: FT-049
title: 'decision-cli: dec verify graph generate (CLI + MCP)'
phase: 2
status: planned
depends-on:
- FT-034
- FT-041
- FT-044
- FT-045
- FT-046
- FT-048
adrs:
- ADR-029
- ADR-030
tests:
- TC-079
- TC-080
- TC-081
- TC-082
domains: []
domains-acknowledged:
  ADR-001: ADR-001 governs the oxi-events crate boundary; FT-049 does not cross or alter that boundary.
  ADR-002: ADR-002 (graph-as-state) governs persistence semantics; FT-049 persists graphs only through the existing slice-2.5 writers (FT-041 + FT-044) which themselves go through the StreamWriter chokepoint.
  ADR-004: ADR-004 (PROV-O) governs session/event lineage; FT-049 opens a dispatch session for the worker invocation and records prov:wasGeneratedBy on the resulting graph.
  ADR-005: ADR-005 (value-stream scope) governs command-time scope; FT-049 runs inside an already-scoped command and does not introduce a new scope check.
  ADR-012: ADR-012 (per-stream working directory discovery) governs CLI entry; FT-049 runs after the working directory is resolved and does not re-discover it.
  ADR-013: ADR-013 (code structure standards) applies workspace-wide; FT-049's code conforms to cargo/clippy/rustfmt and the module-size convention. ADR-013 itself is owned by FT-014.
  ADR-014: ADR-014 (fitness functions tracked as artifacts) is owned by FT-014/FT-015; FT-049 does not author or modify a fitness-function artifact.
  ADR-016: ADR-016 (vertical-slice + compile-time SDP) is migrated by FT-018; FT-049's code is organised under that migration.
  ADR-017: ADR-017 (action-interpretation pairing) is a Slice 2 structural requirement implemented by FT-021; FT-049 produces the action half (GraphProposal/Graph artifact); slice 3's executor will produce the interpretation.
  ADR-018: ADR-018 (VerificationVerdict schema) is a Slice 2 artifact implemented by FT-020; FT-049 neither emits nor consumes verdicts.
  ADR-021: ADR-021 (action-interpretation agreement metric) is a Slice 2 fitness function implemented by FT-024; FT-049's pairing will be measured by the slice-3 executor pairing, not this feature.
  ADR-022: ADR-022 (feedback as a first-class flow class) is a Slice 3 concern implemented by FT-026; FT-049 surfaces Gap proposals but does not route them via the feedback flow in slice 2.6.
  ADR-023: ADR-023 (feedback class controlled vocabulary) is implemented by FT-028; FT-049 produces no feedback artifacts in slice 2.6.
  ADR-024: ADR-024 (feedback lifecycle state machine) is implemented by FT-027; FT-049 produces no feedback artifacts in slice 2.6.
  ADR-025: ADR-025 (blocking vs non-blocking feedback semantics) is implemented by FT-032; FT-049 has no feedback to gate.
  ADR-027: ADR-027 (authority declarations in role catalog) is implemented by FT-030; FT-049 does not introduce or modify a role catalog entry.
---

## Description

The `dec verify graph generate` CLI subcommand and its paired MCP tool `dec_verify_graph_generate`. The user-facing entry point for [ADR-030](ADR-030)'s verify-graph-author role: takes a feature id and a target environment, assembles the bundle, runs [FT-046](FT-046)'s matcher first, invokes [FT-048](FT-048)'s worker only when there is no complete match, presents the proposal for review, and persists on accept through the existing slice-2.5 writers ([FT-041](FT-041) `graph new` + [FT-044](FT-044) `step add`). Inherits the single-handler discipline from [ADR-029](ADR-029).

Level-3 autonomy ([ADR-030](ADR-030) §7): the proposal is shown and accepted on a separate gesture. The CLI does this with an interactive `[y/N]` prompt by default and an explicit `--accept` flag for non-interactive use; the MCP does it with a two-call protocol (`dec_verify_graph_generate` returns the proposal; `dec_verify_graph_accept` writes it). The acceptance handler is a separate tool but shares state through the proposal's `bundle_hash` rather than through server-side session storage — the second call replays the proposal payload and the handler revalidates.

One subcommand → one slice — this slice covers `generate` (which surfaces both the proposal and, via `--accept`, the persistence). The companion `accept` tool exists for MCP's two-call protocol but is a thin shim on the same handler, not a separate verb (it does not create graphs from scratch, only from a proposal payload).

## Functional Specification

### Inputs

- CLI form:
  ```
  dec verify graph generate <FT-NNN> \
    --environment <ENV-NNN> \
    [--accept | --print-only] \
    [--format text|json]
  ```
  - `--accept` writes the proposal immediately (non-interactive mode).
  - `--print-only` shows the proposal and never prompts; useful in scripts that handle acceptance separately.
  - Without either, the CLI prints the proposal and prompts `Accept and persist? [y/N]`.
- MCP form (two tools):
  - `dec_verify_graph_generate` — input `{ feature_id, environment_id }`, returns `GraphProposal` plus a `proposal_token` (the bundle_hash). No persistence.
  - `dec_verify_graph_accept` — input `{ proposal: GraphProposal, proposal_token }`, persists. Revalidates against the current store state; refuses if the candidate set or coverage has changed (the proposal is stale).
- Substrate consumed:
  - [FT-045](FT-045) for coverage computation against the candidate set.
  - [FT-046](FT-046) for match-vs-generate decision (matcher runs *before* the worker).
  - [FT-048](FT-048)'s Python worker (invoked via subprocess like `code-writer`).
  - [FT-041](FT-041)'s `graph new` and [FT-044](FT-044)'s `step add` handlers for persistence (the slice-2.5 writers are the only write path).

### Outputs

- A `GraphProposal` printed in text or JSON.
- On accept: a new `.dec/verify/graph/VG-NNN.ttl` containing the graph header and the proposed steps with their `dec:providesEvidenceFor` predicates set.
- CLI: prints the minted graph id, the on-disk path, and the final coverage report (covered TCs, any residual gaps).
- MCP: returns `{ proposal, proposal_token, coverage_preview }` from `generate`; `{ graph_id, path, coverage_report }` from `accept`.

### State

- No state on `generate` — pure read.
- On `accept`: one new graph file written through [FT-041](FT-041) (then N step-adds through [FT-044](FT-044)). All writes go through `StreamWriter` (SHACL validated). The acceptance session is recorded in PROV-O as `prov:wasGeneratedBy` the verify-graph-author dispatch activity.

### Behaviour

1. Surface adapter constructs a `Request { feature_id, environment_id, mode: Interactive | Accept | PrintOnly }`.
2. Handler resolves the feature and the env (`Error::ArtifactNotFound` on miss).
3. Handler computes [FT-046](FT-046)'s `best_matching_graphs(feature, env)`.
4. If `MatchKind::CompleteSingle` → return that match as a `GraphProposal::Match`; no worker invocation. CLI shows "graph `VG-NNN` already covers this feature in `ENV-NNN`; no new graph needed"; exit 0.
5. Otherwise, assemble the `VerifyGraphAuthorInput` bundle:
   - Feature id + feature_spec body.
   - TCs (resolved through product-cli's existing context bundle).
   - Target env record.
   - Candidate graphs from step 3's `MatchReport.graphs` (the partial-cover candidates).
   - Step vocabulary (the 6 seed kinds plus their `required_ops` and `fields_schema`).
   - `bundle_hash` over the canonical serialisation.
6. Invoke [FT-048](FT-048)'s worker via subprocess (mirroring `features/implement/bundle.rs`'s pattern). Parse stdout as `GraphProposal`.
7. Echo-check `proposal.bundle_hash == request.bundle_hash`; on mismatch → `Error::WorkerProtocolViolation`.
8. **Mode dispatch:**
   - `PrintOnly` → render the proposal, exit 0.
   - `Interactive` → render, prompt `[y/N]`, on `y` proceed to persistence.
   - `Accept` → proceed directly.
9. **Persistence path** (for `new` proposals):
   - Call [FT-041](FT-041)'s `graph new` handler with `verifies = feature`, `environment = env`.
   - For each `ProposedStep`, call [FT-044](FT-044)'s `step add` handler with `step_type`, `fields`, and `provides_evidence_for` (the field that [ADR-028](ADR-028)'s amendment introduced on every step shape).
   - If any step-add fails SHACL or safety → rollback the graph (delete the file, drop the store projection) and surface the error. The acceptance is atomic at the graph granularity.
10. For `match` proposals → no persistence; print the matched graph id.
11. For `gap` proposals → no persistence; print the gap reasoning and exit 0 (a `Gap` is a valid outcome of generation, not a failure).
12. Return final response.

### Invariants

- Single-handler discipline per [ADR-029](ADR-029): CLI and MCP routes converge on one `generate_handler` and one `accept_handler`.
- The matcher always runs before the worker. The worker is never invoked when a complete match exists.
- Persistence reuses the slice-2.5 writers — no new write path. The chokepoint stays intact.
- Acceptance is **all-or-nothing per graph**: a partial graph (graph created but some step-adds failed) must not survive on disk.
- The proposal's `bundle_hash` is the integrity check between worker and handler; mismatches are protocol violations, not low-confidence outputs.
- For MCP, `accept` revalidates against the live store. If the candidate set has changed (e.g. another author wrote a graph between `generate` and `accept`), the acceptance is refused with `Error::ProposalStale`; the caller re-runs `generate`.

### Error handling

- Unknown feature / env → `Error::ArtifactNotFound`; exit 1.
- Worker invocation failure (subprocess non-zero) → `Error::WorkerFailure { detail }`; exit 1.
- Worker protocol violation (malformed JSON, schema mismatch, bundle_hash mismatch) → `Error::WorkerProtocolViolation`; exit 1.
- SHACL violation during step-add → rollback graph, `Error::SchemaViolation`; exit 1.
- Safety violation during step-add → rollback graph, `Error::SafetyViolation`; exit 1.
- MCP accept against stale proposal → `Error::ProposalStale`; exit 1 on CLI, structured error on MCP.

### Boundaries

- **In scope.** `dec verify graph generate` CLI + `dec_verify_graph_generate` MCP + the companion `dec_verify_graph_accept` MCP tool. Bundle assembly, match-first dispatch, Level-3 acceptance flow, atomic-per-graph persistence rollback, integration with [FT-041](FT-041)/[FT-044](FT-044).
- **Out of scope.** Auto-dispatch on feature creation ([FT-050](FT-050)). Auto-acceptance / Level-4 graduation ([ADR-030](ADR-030) §7, slice 3+). Editing an accepted proposal before persistence (acceptance is whole-graph; reject and regenerate to revise). Multi-environment composite generation (one env per call; the user invokes the verb once per env they care about).

## Out of scope

- Auto-dispatch on feature creation ([FT-050](FT-050)).
- Editing the proposal before persistence (regenerate to revise).
- Multi-environment composite proposals.
- Persistence to anywhere but the standard `.dec/verify/graph/` path.
- A separate `dec_verify_graph_accept` *CLI* command — the CLI path inlines accept; MCP needs the two-call protocol for stateless tool use.
