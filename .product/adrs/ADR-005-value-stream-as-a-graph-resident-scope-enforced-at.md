---
id: ADR-005
title: Value stream as a graph-resident scope, enforced at command time
status: accepted
features: []
supersedes: []
superseded-by: []
domains: []
scope: cross-cutting
content-hash: sha256:7b478fec4d210e1811722ba62ae3d58c857bb91d2b12f8717d2ddf4497962a24
---

## Context

Each decision-cli instance is scoped to a single **value stream** — the full chain of processes terminating in a specific value action. Slice 1's stream is `decision-cli-development`, terminating in `va:shipped-feature`.

The question is: where does the scope **live**? Three options:

1. **Configuration file.** A `.dec/config.toml` declares `stream = "decision-cli-development"`. Simple, but the scope is data outside the graph; commands that observe scope would have to read both the config and the graph.
2. **Process flag.** `dec --stream=… <command>`. Easy to override per-invocation, but invites drift and makes audit hard.
3. **Graph-resident artifact.** A persisted `ValueStream` artifact in the orchestration store; every mutation links to it via `dec:inStream`; goal validation runs against its persisted authorized-goals.

The DDD principle says the graph is the system state. Scope is part of the state. Anything else creates a second source of truth.

See `decision-cli-slice-1-bounds.md` §3, §3.4.

## Decision

The value stream is a **graph-resident artifact** persisted at init time, and its scope is **enforced at command time**:

- `dec init` (FT-008) persists a `ValueStream` artifact in the orchestration store.
- Every Session / Goal / Dispatch / Event written through the writer carries a `dec:inStream` triple linking to the active ValueStream (FT-010).
- Every `dec drive <goal> <artifact>` (and slice 1 shorthand `dec implement`) validates the goal verb against the stream's persisted authorized-goals list **before any role dispatches** (FT-010).
- The active stream id is fixed at process start by the discovered `.dec/` directory (see ADR-012); it cannot be changed at runtime.
- Cross-stream artifact insertion attempts are refused with `ScopeError::ForeignStream`.

## Consequences

**Positive:**

- The orchestrator cannot drift outside its declared scope: the boundary is data, not configuration, and validation runs before any role dispatches.
- Scope is auditable. Any artifact in the store names the stream it belongs to.
- Re-pointing an instance at a different stream is impossible without a fresh `dec init` — exactly the friction we want for a load-bearing identity.

**Negative / accepted costs:**

- Slight write overhead per artifact (one extra triple).
- The writer middleware must know the active stream; this is acceptable because the writer is the only mutation chokepoint (FT-001).

**Enforcement:**

- The `dec:inStream` invariant is checked in `product graph check`-style integrity audits (deferred but the slot is reserved).
- Unauthorized-goal refusals produce structured, actionable errors naming the stream's authorized list and the referenced ValueAction.

## Status

Accepted. Governs FT-010, with structural dependence from FT-008 (persists the artifact at init), FT-009 (loads it on store open), and FT-011 (every Session links via `dec:inStream`).
