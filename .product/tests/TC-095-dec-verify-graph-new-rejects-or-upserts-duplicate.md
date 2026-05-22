---
id: TC-095
title: dec_verify_graph_new_rejects_or_upserts_duplicate_id
type: scenario
status: failing
validates:
  features:
  - FT-041
  adrs: []
phase: 1
runner: bash
runner-args: tests/scripts/tc-095-graph-new-duplicate-id.sh
runner-timeout: 120
---

## Purpose

Discovered alongside TC-094 while dogfooding `dec verify` on decision-cli's own implementation (2026-05-22). `dec verify graph new` shares the same disk/store-drift failure mode as `dec verify env new`: duplicate-id detection keys on the on-disk `.ttl`, not on the orchestration store. If the file is missing but the store still has the artifact, a second `graph new --id <same>` silently appends a new set of triples (a second `dec:environment` binding, a second `dec:verifies` link, etc.) instead of upserting or refusing.

This was not directly reproduced during the original discovery because the env bug blocked the dogfood session earlier — but the underlying check is the same shape and the write path is the same (`StreamWriter::commit(Mutation::insert(...))` with no pre-delete of the existing subject), so the bug is structural and must be guarded.

The contract `dec verify graph new` should hold: **for any caller-supplied `--id`, the duplicate check must consult the store; the call either (a) replaces the existing graph's triples atomically, or (b) refuses with a structured error**. It must never silently append, regardless of whether the `.ttl` is present on disk.

Related: TC-094 (env-new variant), FT-041 (this feature).

## Given

- A `.dec/` initialized via `dec init --template engineering-development`.
- Existing environments `ENV-A` and `ENV-B` (both clean).
- An existing graph `VG-T` created via `dec verify graph new --id VG-T --verifies FT-008 --environment ENV-A`.
- The on-disk file `.dec/verify/graph/VG-T.ttl` removed, simulating disk/store drift. The orchestration store still holds the artifact.

## When

```bash
dec verify graph new --id VG-T --verifies FT-008 --environment ENV-B
```

## Then

One of:

1. **Refuse path** — the command exits non-zero with an error naming the existing graph id (e.g. `dec verify graph new: graph VG-T already exists in store; use <rebind-command> to change its environment`), and the store state for `VG-T` is unchanged (still bound to `ENV-A`).
2. **Upsert path** — the command exits 0 and the store contains exactly one `dec:environment` triple for `VG-T`, pointing at `ENV-B`. The previous binding is removed.

In either case:

- `dec verify graph list` succeeds and shows `VG-T` exactly once.
- `dec verify graph show --id VG-T` returns deterministic content with a single `environment` field.
- Existing steps appended to `VG-T` remain intact (the upsert affects only the graph's own triples, not the steps it references).

## Notes

- The accept-or-refuse decision is for FT-041's owners; the test only rejects silent-append.
- If upsert is chosen, consider whether the steps list should also be cleared or preserved. The test as written assumes preserved (id stability == step stability); change this expectation if the feature decides otherwise.
- A `dec verify graph rebind` command would be the natural complement if refuse is chosen — without it, recovery from accidental wrong-env binding requires hand-editing the store.
- The fix likely shares code with TC-094's; both are duplicate-detection-keyed-on-disk bugs.
