---
id: TC-211
title: Migration tool is idempotent on already-migrated store
type: invariant
status: passing
validates:
  features:
  - FT-112
  adrs: []
observes:
- graph
phase: 4
runner: cargo-test
runner-args: tc_211_migration_is_idempotent
runner-timeout: 60
last-run: 2026-05-29T13:41:32.311273790+00:00
last-run-duration: 0.6s
---

## Description

Operators can safely re-run `dec _migrate-env-to-bench` if they
think the prior run was incomplete, lost partway through, or if
they're scripting the migration into a pre-flight. Idempotence
means "no observable diff on the second call." Without this,
a stale or partial run gives the operator no way to recover
without manual store inspection.

## Acceptance Criteria

Cargo test:

1. Build the same fixture as TC-210 (1 ENV instance, 2
   VerificationGraph quads, 1 VGR, 1 control).
2. Run the migration once. Snapshot the store's quad set into
   `snapshot_1` (sorted, deterministic ordering).
3. Run the migration a second time on the post-migration store.
   Snapshot into `snapshot_2`.
4. Assert `snapshot_1 == snapshot_2` (byte-exact equality on the
   sorted N-Quads serialization).

Also verify the second-call return value reports `rewritten: 0`
(zero IRIs touched on the idempotent re-run) so an operator
sees clear evidence the second call was a no-op.