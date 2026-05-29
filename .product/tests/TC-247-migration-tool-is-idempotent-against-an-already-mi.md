---
id: TC-247
title: Migration tool is idempotent against an already-migrated store and reports zero rewrites
type: invariant
status: passing
validates:
  features:
  - FT-117
  adrs: []
observes:
- stdout
- exit-code
phase: 4
runner: bash
runner-args: tests/scripts/tc-247-migrate-cli-idempotent.sh
runner-timeout: 60
last-run: 2026-05-29T18:42:47.594980702+00:00
last-run-duration: 0.2s
---

## Description

Re-running `dec _migrate-env-to-bench` against a fresh-init
workdir (which has no ENV vocabulary) OR an already-migrated
workdir must produce a clean no-op signal. Without this,
operators can't safely script the migration into pre-flight
checks or recover from a half-executed run.

## Acceptance Criteria

Bash test:

1. Compose a temp workdir with NO ENV vocabulary (fresh init,
   already-migrated state). Capture the orchestration store's
   sorted N-Quads as `snapshot_pre`.
2. Run `dec _migrate-env-to-bench --workdir <temp>`. Assert
   exit 0.
3. Assert stdout contains "rewrote 0 quads" (or the
   established equivalent phrase).
4. Capture the store's sorted N-Quads as `snapshot_post1`.
   Assert `snapshot_pre == snapshot_post1` (byte-equal).
5. Repeat the migration; capture `snapshot_post2`. Assert
   equal to both prior snapshots.
6. Verify dry-run prints the same "0 quads" report:
   `dec _migrate-env-to-bench --workdir <temp> --dry-run`
   exit 0, stdout contains `[DRY-RUN]` and "0 quads".