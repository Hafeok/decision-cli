---
id: FT-158
title: 'decision-cli: dec archetype list/show/promote/demote/reframe-instance CLI + MCP twin'
phase: 5
status: planned
depends-on:
- FT-147
- FT-149
adrs:
- ADR-081
- ADR-085
tests: []
domains:
- api
domains-acknowledged: {}
---

## Description

The `dec archetype` CLI namespace — list, show, promote, demote, reframe-instance — with MCP twin per [ADR-029](ADR-029) and the list/show totality invariant from [ADR-081](ADR-081). This is the operator-facing surface for the archetype catalog landed in [FT-147](FT-147)..[FT-156](FT-156).

Key constraints from prior ADRs that this slice enforces:

- **list/show totality** ([ADR-081](ADR-081)): every IRI returned by `dec archetype list` resolves via `dec archetype show`. Registry-driven; canonical projection function shared across both verbs.
- **promote/demote are CLI-only** ([ADR-085](ADR-085) §3): MCP twin intentionally does *not* expose promote/demote. The MCP surface exposes list, show, and reframe-instance only.
- **CLI shape** ([ADR-011](ADR-011)): namespaced subcommands, single `dec` binary.
- **promote evidence gate** ([ADR-085](ADR-085) §1): `promote` refuses with E110 when the four evidence requirements are not met.
- **reframe-instance is the only mutation path for frozen instances** ([FT-149](FT-149) freeze gate): direct writes to frozen instances refuse with E020.

## Functional Specification

### Inputs

- `Archetype`, `ApplicationContract`, `InfrastructureContractTemplate`, `InfrastructureContractInstance`, `TaskType`, `Cell`, `SeamAudit`, `ArchetypeAudit`, `RegressionEvidence`, `TaskTypeCandidate` — the full archetype-layer ontology from FT-147..FT-156.
- The existing clap command tree at `crates/decision-cli/src/main.rs` and `crates/decision-cli/src/features/*/cli.rs`.
- The `cli_pairing.rs` registry from ADR-081.
- The existing MCP server scaffolding ([FT-034](FT-034)).

### Outputs

**Clap subcommands** under `dec archetype`:

```
dec archetype list                                  # → IRI list + status + title + variance
dec archetype list --status candidate|standard|quarantined
dec archetype list --format json|table              # JSON for ADR-081's totality check
dec archetype show <iri>                            # full archetype detail
dec archetype show <iri> --include contracts|task-types|audits|instances|evidence
dec archetype promote <iri> --reviewer <name> --reason <reason>     # CLI-only; ADR-085
dec archetype demote <iri> --reviewer <name> --reason <reason>      # CLI-only; ADR-085
dec archetype reframe-instance <instance-iri> --reviewer <name> --reason <reason>   # mutation gate from FT-149
```

**Canonical projection** at `crates/decision-cli/src/core/graph/archetype.rs::project`:

Implements the ADR-081 query-shape rule. Both list and show feed the same projection function; the field set is identical; the SPARQL bodies bind `?s` differently (list enumerates `?s a dec:Archetype`; show binds `?s` to the input IRI) but the projection clause is shared. This makes the "list permissive / show strict" drift class structurally impossible.

**MCP twin** at `crates/decision-cli/src/features/mcp/archetype.rs`:

- `mcp__product__product_archetype_list` — same shape as CLI `list`.
- `mcp__product__product_archetype_show` — same shape as CLI `show`.
- `mcp__product__product_archetype_reframe_instance` — same shape as CLI `reframe-instance`.

Notably absent: `promote`, `demote`. Per ADR-085 §3 these are human decisions; the MCP path is not the right shape.

**Registry update** at `crates/decision-cli/src/core/cli_pairing.rs`:

```rust
register(("archetype", "list", "show", None));
```

The ADR-081 platform TC walks this registry; the new pair is covered automatically.

**Promote command implementation:**

1. Read the archetype.
2. Run the four evidence checks from ADR-085 §1: ≥3 instances, every SeamAudit at `monolith_bar: Passes`, EVIDENCE.md filled, `application_contract_held_invariant: true`.
3. If any check fails → E110 (`E110_ArchetypePromotionEvidenceMissing`) with a per-check diagnostic.
4. If all pass → create the `ArchetypePromotion { decision: approved, ... }` record + flip `Archetype.status: standard` via the typed GraphWriter path that bypasses the E020 mutation gate.
5. Emit `dec:ArchetypePromoted` event.

**Demote command implementation:**

Lower bar per ADR-085 §4. Required: `--reviewer` + `--reason`. Creates `ArchetypePromotion { requested_status: candidate, decision: approved, ... }` record + flips `Archetype.status: candidate`.

**Reframe-instance implementation:**

The only sanctioned path for mutating frozen InfrastructureContractInstance per [FT-149](FT-149). Creates a `dec:InstanceReframe` audit record (date, reviewer, reason, prior IRI snapshot), unfreezes the instance to Draft, allows mutation. The next freeze creates a new audit record. The reframe history is queryable via `dec archetype show <instance-iri> --include reframe-history`.

**Show command details:**

Default output: title + status + variance + counts (instances, task-types, audits).

With `--include` flags:
- `contracts` → application contract summary + infrastructure template summary.
- `task-types` → list of TaskType IRIs grouped by family.
- `audits` → list of ArchetypeAudits + SeamAudits with monolith_bar status.
- `instances` → list of InfrastructureContractInstance IRIs + customer_id + status.
- `evidence` → EVIDENCE.md content + W104 readiness flag.

**Test coverage:**

- list-show round-trip on empty store: `dec archetype list` returns empty; total trivially.
- list-show round-trip on populated store: with the decision-cli archetype from FT-160 present, list returns its IRI; show resolves it; ADR-081 totality holds.
- promote evidence gate: candidate archetype missing one piece → E110 with the missing piece named.
- promote success: candidate archetype with all four pieces → status flips to standard; ArchetypePromotion record created.
- demote success: standard archetype + reviewer + reason → flips to candidate; audit record created.
- reframe-instance: frozen instance + reviewer + reason → status flips to Draft; reframe record created.
- MCP twin parity (list, show, reframe-instance only): identical output to CLI; promote/demote intentionally absent and the MCP request returns "method not found" (not "unauthorized").
- cli_pairing registry: ADR-081 platform TC discovers the new pair without manual update.

### State

- **New on-disk:** `features/archetype/cli.rs`, `features/archetype/mcp.rs`, `features/archetype/promote.rs`, `features/archetype/demote.rs`, `features/archetype/reframe.rs`, `core/graph/archetype.rs` (the projection function).
- **Modified on-disk:** `main.rs` (clap registration), `core/cli_pairing.rs` (registry update), `features/mcp/server.rs` (MCP method registration).

### Behaviour

1. **Cluster dispatch via `add-cli-subcommand`** ([FT-142](FT-142)). The slice rides the established CLI-subcommand cluster — clap args + handler + integration test + MCP twin.
2. **list / show share the canonical projection**. No drift possible by construction.
3. **promote runs evidence checks**. Refuses or emits the promotion record.
4. **reframe-instance is the only frozen-instance mutation path**. SHACL chokepoint refuses any other write path.

### Invariants

- **list/show totality** (ADR-081). Enforced by canonical projection.
- **promote/demote CLI-only** (ADR-085). MCP twin absent for these verbs.
- **Frozen-instance mutation only via reframe-instance**. Other paths refuse with E020.
- **Promotion records are immutable**. Once `decision: approved | rejected`, no mutations.
- **Audit trail survives demotion**. Demoting an archetype does not delete the original ArchetypePromotion record.

### Error handling

- **E110** — promote evidence missing; lists which of the four pieces failed.
- **E020** — non-reframe mutation of a frozen instance.
- **Promote on already-standard archetype** → no-op with informational outcome (not an error).
- **Demote on already-candidate archetype** → no-op with informational outcome.
- **Reframe-instance on a Draft instance** → informational outcome ("already mutable; no reframe needed"); no record created.

### Boundaries

- **In scope.** Five CLI subcommands; three MCP twin methods (list, show, reframe-instance); canonical projection; cli_pairing registry update; promote evidence check (E110); demote audit record; reframe-instance audit record + freeze-gate enforcement; eight TCs.
- **Out of scope.** `dec drive archetype <id>` — FT-159. `dec archetype extract --from <repos>` (the pattern-extractor CLI entrypoint) — likely a future slice; FT-156 ships the worker but not the CLI verb. Multi-archetype operations (`dec archetype merge`, `dec archetype split`) — out of v1. Archetype versioning — uses amendment shape when needed.

## Out of scope

- `dec drive archetype` — FT-159.
- `dec archetype extract` CLI verb (future).
- Multi-archetype operations.
- Archetype versioning.
- LLM-assisted promotion review.
