---
id: TC-094
title: dec_verify_env_new_rejects_or_upserts_duplicate_id
type: scenario
status: passing
validates:
  features:
  - FT-038
  adrs: []
phase: 1
runner: bash
runner-args: tests/scripts/tc-094-env-new-duplicate-id.sh
runner-timeout: 120
last-run: 2026-05-22T10:32:43.401516252+00:00
last-run-duration: 0.2s
---

## Purpose

Discovered while dogfooding `dec verify` on decision-cli's own implementation (2026-05-22). `dec verify env new` keys its duplicate-id detection on the **on-disk `.ttl` file**, not on the orchestration store. If the file is missing but the store still has the artifact (which happens after an accidental `rm`, gitignore mismatch, test cleanup, or any partial restore), a second `env new --id <same>` silently appends a new `dec:allowedOps` list to the existing env in the store instead of upserting or refusing.

The persisted env then has two `dec:allowedOps` heads, after which:

- `dec verify env show` returns one of the two lists nondeterministically.
- `dec verify env list` errors with `env "...ENV-NNN" declares 2 dec:allowedOps heads`.
- Any subsequent `dec verify step add` against a graph bound to that env trips the FT-037 safety check with the stale (old) list — even though the on-disk `.ttl` is authoritative and contains the new list.

The corrupt state cannot be recovered through any MCP/CLI path; there is no delete, no upsert, no rebind. The only workaround is hand-editing `.dec/store/orchestration.nq` (which is the wrong layer) or wiping the store (which destroys session/event history).

The contract `dec verify env new` should hold: **for any caller-supplied `--id`, the duplicate check must consult the store (the authoritative source); the call either (a) replaces the existing env's triples atomically, or (b) refuses with a structured error**. It must never produce a multi-headed env in the store, regardless of whether the `.ttl` is present on disk.

Related: FT-038 (this feature), FT-041 (same bug shape for graph-new — see TC-095), FT-039 (list resilience downstream — see TC-096).

## Given

- A `.dec/` initialized via `dec init --template engineering-development`.
- An existing environment `ENV-T` created via `dec verify env new --id ENV-T --type ephemeral-tempdir --safety-class isolated --allowed-ops shell,filesystem`.
- The on-disk file `.dec/verify/env/ENV-T.ttl` removed (e.g. `rm`), simulating disk/store drift. The orchestration store still holds the artifact.

## When

```bash
dec verify env new --id ENV-T --type ephemeral-tempdir --safety-class isolated --allowed-ops shell,filesystem,sparql-local
```

## Then

One of:

1. **Refuse path** — the command exits non-zero with an error naming the existing env id (e.g. `dec verify env new: environment ENV-T already exists in store; use <update-command> to modify`), and the store state for `ENV-T` is unchanged.
2. **Upsert path** — the command exits 0 and the store contains exactly one `dec:allowedOps` triple for `ENV-T`, pointing at the new list `(shell filesystem sparql-local)`. The previous list is removed.

In either case:

- `dec verify env list` succeeds (no "declares N dec:allowedOps heads" error).
- `dec verify env show --id ENV-T` returns deterministic content.
- `dec verify step add` against a graph bound to `ENV-T` reads the canonical list (the one matching the new `.ttl`).

## Notes

- The decision between refuse and upsert is for FT-038's owners to settle; the test should accept either, but reject the silent-append behaviour.
- If upsert is chosen, the rewrite must be transactional through the SHACL chokepoint (`StreamWriter`), not a direct store mutation.
- The root cause is duplicate-detection keyed on the `.ttl` file instead of the store; fixing the detection in one place may close several adjacent issues.
- Same root-cause failure mode applies to `dec verify graph new`; see TC-095.