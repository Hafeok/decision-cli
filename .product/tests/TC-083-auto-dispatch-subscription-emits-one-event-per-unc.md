---
id: TC-083
title: auto-dispatch subscription emits one event per uncovered feature-env pair
type: scenario
status: unimplemented
validates:
  features: []
  adrs: []
phase: 1
---

## Premise

The subscription is enabled with `auto_dispatch = true` and `envs = ["ENV-1", "ENV-2"]`. A new feature `FT-L` is created with TCs `[T1, T2]`. No graph covers `FT-L` in either env.

## Acceptance Criteria

- The subscription emits exactly two `VerifyGraphAuthorDispatchEvent`s — one for `(FT-L, ENV-1)`, one for `(FT-L, ENV-2)`.
- Each event carries a distinct `bundle_hash` (the bundles differ on `target_environment`).
- Each event references the originating feature-create event via `triggered_by_event_id`.
- No events are emitted for envs not listed in `envs`.
- If a third env exists in the catalog but is not in `envs`, no event fires for it.

## Notes

Per-env independence is the subscription's contract: a feature gets one proposal opportunity per configured env. The matcher will short-circuit later if coverage already exists in some env.
