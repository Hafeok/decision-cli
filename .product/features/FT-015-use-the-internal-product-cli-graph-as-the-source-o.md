---
id: FT-015
title: Use the Internal product-cli Graph as the Source of Truth for Code Quality Rules
phase: 1
status: in-progress
depends-on: []
adrs:
- ADR-014
- ADR-013
tests:
- TC-016
- TC-017
domains: []
domains-acknowledged:
  ADR-012: FT-015 ships no dec command surface; per-stream working directories are not exercised by this feature's deliverables.
  ADR-005: FT-015 modifies the .product/ graph and scripts/ — both repo-level, not value-stream-scoped artifacts. ValueStream enforcement does not apply.
  ADR-001: FT-015 ships convention and documentation only — no oxi-events code changes, so the SDP boundary is not exercised by this feature.
  ADR-004: FT-015 is documentation and scaffolding — no Session, Event, or Bundle artifact is produced, so PROV-O linkage does not apply.
  ADR-002: FT-015 ships scripts, TCs, and CLAUDE.md edits — no orchestration store mutations, so the graph-as-state principle is not exercised.
---

## Description

Adopt the convention described in ADR-014: decision-cli's *internal* product-cli graph (the one rooted at `.product/` in this repository) is the single source of truth for code-quality rules and other architectural fitness functions. This feature is the one-time scaffolding work that makes that adoption concrete — authoring the first batch of rules-as-ADRs, validating that `product verify --platform` picks them up, and documenting the lifecycle in CLAUDE.md so future contributors and agent sessions know where rules live.

The feature ships when (a) at least one rule (ADR-013 — code quality) is governed end-to-end through the internal graph, (b) `product verify --platform` runs the linked TCs as part of CI, and (c) the lifecycle for adding/changing/retiring a rule is documented in CLAUDE.md with a worked example.

## Functional Specification

### Inputs

- The existing `.product/` setup created during decision-cli's bootstrap (already present as of FT-006/FT-007 work).
- ADR-014 (this feature's governing decision) defining the rules-in-the-internal-graph convention.
- ADR-013 (and any subsequent rule ADRs) as the first inhabitants of the rules surface.
- CLAUDE.md as the destination for the lifecycle documentation block.

### Outputs

- An updated `.product/` graph in which at least one cross-cutting rule ADR + its TCs are present, linked, and surfaced to every feature's context bundle.
- A new "Rules live in `.product/`" section in CLAUDE.md describing how to add, change, and retire a rule.
- An entry in CI configuration that runs `product verify --platform` and treats exit 1 as failure, exit 2 as warning (and surfaces it on the PR).
- A worked example in `.product/prompts/` or CLAUDE.md showing the `product author adr` → `product request apply` → `product verify --platform` flow for landing a new rule.

### State

The persistent state is the graph itself: `.product/adrs/`, `.product/tests/`, and the request log in `.product/requests.jsonl`. Every rule landing produces appendable, hash-chained log entries — the audit trail is automatic via product-cli FT-042.

### Behaviour

- New rule lands through ordinary `product author adr` or `product request apply` flow. No special command exists; the convention is editorial.
- `product context FT-XXX --depth 2` for any decision-cli feature includes the cross-cutting rule ADRs in its bundle automatically (existing product-cli behaviour for `scope: cross-cutting`).
- `product verify --platform` invocation in CI runs every TC whose `validates.adrs` references a cross-cutting ADR. Exit codes propagate per ADR-014 §"Enforcement is automated."
- `product drift check` against a rule ADR with `source-files` populated (e.g. `scripts/checks/file-length.sh`) detects changes to the script that are not paired with an ADR amendment.

### Invariants

- Every rule in decision-cli is reachable as `(ADR, [TC...])` in `.product/`. No rule lives only in a CONTRIBUTING.md, README admonition, or commit-log convention.
- Every cross-cutting ADR has at least one TC with a `runner` configured. (`product preflight` would surface a missing runner per FT-058's enforcement in product-cli.)
- Every rule TC's `validates.features` is empty (rules are cross-cutting by definition under this feature's convention).
- The CLAUDE.md "Rules live in `.product/`" section stays in lockstep with ADR-014. Drift between the two surfaces in `product drift check` if ADR-014 is added to a list of `source-files` it governs.

### Error handling

- Authoring a rule ADR without `scope: cross-cutting` produces a structural inconsistency surfaced by `product graph check` (the rule is missing from cross-cutting bundles).
- Authoring a rule TC without `runner` config blocks the feature from transitioning to in-progress (per FT-058 in product-cli).
- Authoring a rule ADR with a `domain` not in `.product/config.toml`'s `[domains]` table fails validation at `product request apply` time.

### Boundaries

- This feature **does not** import rules from the external product-cli project. decision-cli's rules are its own (see ADR-014 §"Why the internal graph and not the external product-cli repo").
- This feature **does not** introduce a new artifact type. Rules are existing ADRs and TCs; the only thing this feature adds is convention.
- This feature **does not** modify product-cli. The cross-cutting-ADR-in-bundle and verify-platform behaviours already exist upstream and are consumed as-is.
- This feature **does not** define every code-quality rule decision-cli will ever have. It ships the convention and the first rule (ADR-013). Subsequent rules land as ordinary ADR authoring sessions.

## Out of scope

- A dedicated `dec rules ...` CLI subcommand. The convention is "use product-cli the way you already do." A separate verb would imply a separate artifact type and defeat ADR-014's "no second system" principle.
- A web UI or dashboard for rules. The same `.product/` markdown surface and `product status` view that serves features serves rules.
- Cross-stream rules. Per `decision-cli-slice-1-bounds.md` §3.6, cross-stream artifact propagation is deferred. Each value stream that uses decision-cli is responsible for its own internal rules graph until that lands.
- Migrating "rules" that don't yet exist anywhere. We are not on a remediation campaign — we are setting up the surface so future rules land in the right place.

## Derivation

This feature operationalises ADR-014. It draws lineage from product-cli's ADR-024 ("Architectural Fitness Functions — Continuous Metric Tracking") and ADR-025 ("Concern Domains — ADR Classification and Cross-Cutting Scope"), both of which establish the upstream machinery — cross-cutting scope, fitness-function tracking, and the verify-platform pipeline — that decision-cli relies on for this convention to work.
