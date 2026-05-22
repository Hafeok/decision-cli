---
id: FT-061
title: 'decision-cli: Dispatcher capability resolution (default_capability path, no escalation)'
phase: 2
status: planned
depends-on:
- FT-054
- FT-055
- FT-058
- FT-060
adrs:
- ADR-001
- ADR-002
- ADR-004
- ADR-005
- ADR-008
- ADR-012
- ADR-013
- ADR-014
- ADR-015
- ADR-016
- ADR-017
- ADR-018
- ADR-020
- ADR-021
- ADR-022
- ADR-023
- ADR-024
- ADR-025
- ADR-027
- ADR-033
- ADR-034
- ADR-035
- ADR-036
- ADR-037
tests:

- TC-107
- TC-108
domains:
- api
- error-handling
- observability
domains-acknowledged: {}
---

## Description

Replace the dispatcher's hardcoded model selection with a graph-driven capability resolution step per [ADR-033](ADR-033). Given a role and a bundle, the dispatcher reads the role's active `dec:RoleBinding` ([FT-055](FT-055)), resolves the `default_capability` to a concrete `dec:Capability` artifact ([FT-054](FT-054)), and injects the resolved `(endpoint, model_identifier, parameters)` triple into the dispatch payload. The worker consumes this triple via the `ModelRouter` from [FT-060](FT-060).

This feature ships the *default-capability* path. Escalation — running additional steps when triggers fire — lives in [FT-062](FT-062). After this feature lands, every dispatch uses graph-resolved model selection for its first attempt; escalation behavior is unchanged from today (no escalation).

## Functional Specification

### Inputs

- The `dec:Capability` schema and active-by-id query from [FT-054](FT-054).
- The `dec:RoleBinding` schema and `active_for_role` query from [FT-055](FT-055).
- The catalog seeded by [FT-058](FT-058) (so a `dec init`'d store has bindings for every active role).
- The existing dispatcher in `crates/decision-cli/src/core/harness/` (slice 1 path) and `features/ft_021_*` (verify pairing).
- The dispatch event payload schema from [FT-003](FT-003) / [FT-004](FT-004).

### Outputs

- New module `core::dispatcher::capability_resolver`:
  ```rust
  pub struct ResolvedCapability {
      pub capability_id: String,
      pub capability_version: u32,
      pub endpoint: Endpoint,            // from FT-054
      pub model_identifier: String,
      pub max_output: u32,
      pub supports_tool_calling: bool,
      pub configurable_effort: bool,
      pub binding_version: u32,          // version of the RoleBinding that produced this
  }
  
  pub fn resolve_default_capability(
      graph: &impl GraphReader,
      role_id: &str,
  ) -> Result<ResolvedCapability, ResolverError>;
  ```
- Extended dispatch payload schema (the JSON the worker receives, per [FT-003](FT-003)):
  ```json
  {
    "dispatch_id": "...",
    "bundle_markdown": "...",
    "bundle_hash": "...",
    "role_id": "verifier",
    "endpoint": "scaleway",
    "model_identifier": "qwen3-coder-30b-a3b-instruct",
    "parameters": {
      "temperature": 0.0,
      "max_tokens": 32000,
      "reasoning_effort": null
    },
    "capability_ref": {"id": "code-writer", "version": 1},
    "binding_ref": {"role_id": "verifier", "version": 1}
  }
  ```
- Session record extension (uses fields already on `dec:SessionRecord` plus [ADR-004](ADR-004)'s PROV-O):
  - `dec:capability` (object property, optional; range `dec:Capability`) — pinned at write time so reproducibility includes capability identity.
  - The session also records `dec:capability_version` (xsd:integer) and `dec:binding_version` (xsd:integer) as denormalised columns for fast queries.
- Compatibility check at resolution time:
  - If the resolved capability has `supports_tool_calling = false` and the role's worker requires tool calling (currently `implementer`, `verifier`), the resolver returns `ResolverError::IncompatibleCapability` and the dispatch is refused. This catches misconfigured catalogs before a worker invocation that would silently produce broken output.

### State

- The dispatcher gains a per-process LRU cache for `active_for_role(role_id) -> RoleBinding` keyed by role_id with size 16 (the number of distinct roles is small). The cache is invalidated when a new `RoleBinding` is written via `GraphWriter` (the writer publishes an invalidation event on the same channel `oxi-events` subscriptions use). Cold cache is fine for slice 2 — dispatches per second are low. If profiling shows the cache miss matters, the invalidation hook is already in place.

### Behaviour

1. The dispatcher receives a `(role_id, bundle)` pair from `dec implement`, `dec verify`, or auto-dispatch ([FT-050](FT-050)).
2. Before constructing the dispatch payload, the dispatcher calls `resolve_default_capability(graph, role_id)`:
   - Look up the active `RoleBinding` for the role.
   - Read the `default_capability` reference.
   - Look up the `Capability` artifact; verify `status ≠ eol`.
   - Verify tool-calling compatibility per the role.
   - Construct `ResolvedCapability`.
3. The dispatcher constructs the dispatch payload with the resolved fields. `parameters.temperature` is read from the capability's stored parameters (default 0.0); `parameters.max_tokens` is `min(bundle.requested_max_tokens, capability.max_output)`; `parameters.reasoning_effort` is left `null` here — [FT-063](FT-063) populates it when `configurable_effort = true`.
4. The dispatch event is published per [FT-003](FT-003).
5. After the worker returns, the session record is written with `dec:capability`, `dec:capability_version`, `dec:binding_version` recorded.

### Invariants

- Every session written after this feature lands cites the capability that ran it (pinned by version, not just id).
- The dispatcher does not call the model directly — it resolves and delegates. The worker layer (via [FT-060](FT-060)) is the only place model calls happen.
- The cache is read-through and invalidated on write; no stale binding can be used after a catalog update.
- The dispatcher refuses to dispatch when no active binding exists for a role (rather than falling back to a hardcoded model). This is a deliberate hard stop — silent fallback masks misconfiguration per [ADR-037](ADR-037).
- The dispatcher refuses to dispatch when the resolved capability is incompatible with the role's worker tool requirements.

### Error handling

- No active binding for the role → `ResolverError::NoActiveBinding { role_id }`; dispatch refused; error surfaces through the CLI as "role X has no active binding; run `dec init` or supersede manually".
- Resolved capability is `status = eol` → `ResolverError::CapabilityEol { id }`; dispatch refused.
- Tool-calling incompatibility → `ResolverError::IncompatibleCapability { role_id, capability_id, reason }`; dispatch refused.
- Graph read error (corrupted store) → bubbles as `ResolverError::GraphError`; dispatch refused.

### Boundaries

- **In scope.** `capability_resolver` module, payload extension, session record extension, compatibility check, cache, error handling.
- **Out of scope.** Escalation — [FT-062](FT-062).
- **Out of scope.** Reasoning_effort population — [FT-063](FT-063).
- **Out of scope.** Worker layer changes — [FT-060](FT-060) for the verifier; [FT-064](FT-064) for the migration.

## Out of scope

- Per-bundle binding override (rejected by [ADR-035](ADR-035) — bindings are per-role).
- Dynamic re-resolution mid-dispatch (escalation is the structured way to change capability mid-dispatch; this is [FT-062](FT-062)).
- A `dec capability resolve <role>` CLI command (introspection is via `dec binding show <role>` once a UX feature_spec adds it; for slice 2 the SPARQL helpers are enough).
- Pre-emptive batch resolution for many dispatches (one resolution per dispatch is fine at slice-2 throughput).
