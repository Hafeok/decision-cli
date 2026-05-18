---
id: ADR-010
title: Explicit human triggering in slice 1
status: accepted
features: []
supersedes: []
superseded-by: []
domains: []
scope: feature-specific
content-hash: sha256:2419791e8bb3f5d8eb77e9c3d864e8ee360e9e639d13248585dd358bd33c87e2
---

## Context

The full DDD orchestration vision is reactive: product-cli emits events on artifact changes, oxi-events delivers them, decision-cli's policy layer picks the next role to dispatch, and the system runs forward without human intervention until a checkpoint. This is the autonomy ladder's upper rungs.

Slice 1 is **not** that. The risks slice 1 proves are mechanical (Rust + Oxigraph carries the load, bundle-as-SPARQL pattern is natural, stateless worker contract holds, oxi-events extracts cleanly under SDP). Adding a reactive loop on top would force premature commitment to:

- Policy artifacts (which role runs next, under what conditions).
- Model catalog (which model serves which role).
- Interpretation pairing (the verification session that gates an action's output).
- Feedback flow lifecycle (gap / contradiction / unimplementable / scope-issue routing).
- product-cli event emission (the other side of the reactive wire).

Each of those is a significant design effort. Doing them in parallel with slice 1's mechanical proof would either delay slice 1 or rush the policy work.

See `decision-cli-slice-1-bounds.md` §6.1 (in scope: explicit human triggering), §6.2 (deferred), §6.3 (why this scope).

## Decision

Slice 1 dispatches roles **only by explicit human trigger**.

- `dec implement FT-XXX` starts the implementer loop on demand.
- No watcher, scheduler, or reactive subscription triggers a dispatch.
- The v0 seed subscriptions ("dispatch available for code-writer," "code-writer dispatch completed") exist to demonstrate the substrate works end-to-end; they do **not** automatically chain to further dispatches.

The shorthand `dec implement` is preserved as a convenience even in later slices — for any single-role direct dispatch, the verb form of the role is a valid shortcut for `dec dispatch role <role> <artifact>`.

## Consequences

**Positive:**

- Slice 1 ships without committing to policy / model-catalog / interpretation-pairing designs.
- The human-in-the-loop pattern is concretely tested before automation is layered on.
- Failure modes are obvious: a dispatch happens because a human asked for it; if something goes wrong, the cause is localised.

**Negative / accepted costs:**

- The reactive value proposition of the framework is not yet realised end-to-end in slice 1.
- Slice 2 will introduce the reactive loop and the policy artifact — that work is on the critical path immediately after slice 1.

**Slice 2 changes:**

- product-cli emission of oxi-events events.
- First reactive subscription: `dec implement` triggered by a feature_spec landing in `status: ready`.
- Policy artifact: which roles auto-trigger, under what conditions.

## Status

Accepted. Governs FT-011 (implementer dispatch initiated by `dec implement FT-XXX`) and FT-012 (CLI surface includes `implement`, excludes `drive` / `watch` / `schedule`).
