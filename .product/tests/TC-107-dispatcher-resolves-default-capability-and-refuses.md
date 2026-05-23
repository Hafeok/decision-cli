---
id: TC-107
title: Dispatcher resolves default_capability and refuses when no active binding
type: exit-criteria
status: passing
validates:
  features:
  - FT-061
  adrs: []
phase: 2
runner: cargo-test
runner-args: -p decision-cli --test capability_resolver
runner-timeout: 180
last-run: 2026-05-23T16:10:16.282180202+00:00
last-run-duration: 0.2s
---

## Description

Scenario: `core::dispatcher::capability_resolver::resolve_default_capability` reads the active `dec:RoleBinding` for a role, walks to the `dec:Capability` artifact, and produces a `ResolvedCapability`. It refuses to resolve when there is no active binding, when the resolved capability is EOL, or when the capability is incompatible with the role's worker tool requirements. The resolved triple is injected into the dispatch payload with `capability_ref` and `binding_ref` version pins.

The runner is `cargo-test` against a temp orchestration store seeded with the PRD §6.2 bindings.

Acceptance:

1. **Happy path — verifier.** Seed the catalog. Call `resolve_default_capability(graph, "verifier")`. Assert the result has `capability_id = "code-writer"`, `endpoint = Scaleway`, `model_identifier = "qwen3-coder-30b-a3b-instruct"`, `capability_version = 1`, `binding_version = 1`.
2. **Happy path — architect.** Call for `"architect"`. Assert `capability_id = "standard-reasoning"`, `endpoint = Scaleway`, `configurable_effort = true`.
3. **No active binding.** Drop the `architect` binding (set `active = false`). Assert `resolve_default_capability(graph, "architect")` returns `ResolverError::NoActiveBinding { role_id: "architect" }`.
4. **EOL capability.** Mutate `code-writer` capability to `status = "eol"`. Assert resolve for `verifier` returns `ResolverError::CapabilityEol { id: "code-writer" }`. Restore status afterwards.
5. **Incompatible capability.** Mutate `code-writer` to `supports_tool_calling = false`. Resolve for `implementer` (whose worker requires tool calling). Assert `ResolverError::IncompatibleCapability { role_id: "implementer", capability_id: "code-writer", reason: "supports_tool_calling=false" }`.
6. **Cache invalidation.** Resolve for `verifier` (warms cache). Write a new active `dec:RoleBinding` for verifier with `default_capability = standard-reasoning-frontier` and `supersedes` to prior. Resolve again; assert the new binding is observed (no stale cached result).
7. **Dispatch payload shape.** Issue a `dec verify` for a feature with the catalog seeded. Capture the dispatch event payload (via [FT-003](FT-003)'s outbox). Assert the payload JSON has top-level fields `endpoint`, `model_identifier`, `parameters`, `capability_ref: {id, version}`, `binding_ref: {role_id, version}`.
8. **Session record cites capability.** After dispatch completes, query the session record. Assert `?session dec:capability ?cap` and `?cap dec:capability_id "code-writer"`; assert `?session dec:capability_version 1` and `?session dec:binding_version 1`.

⟦Σ:Types⟧{
  ResolverError ≜ NoActiveBinding | CapabilityEol | IncompatibleCapability | GraphError
}

⟦Γ:Invariants⟧{
  ∀ role: resolve(graph, role).is_ok ⇒ payload(role).endpoint = resolved.endpoint
  ∀ session: session.capability_version > 0 ∧ session.binding_version > 0
}