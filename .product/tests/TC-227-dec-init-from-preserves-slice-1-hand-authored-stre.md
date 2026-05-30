---
id: TC-227
title: dec init --from preserves slice-1 hand-authored stream behaviour bit-identically
type: invariant
status: passing
validates:
  features:
  - FT-114
  adrs: []
observes:
- file
- exit-code
phase: 4
runner: bash
runner-args: tests/scripts/tc-227-init-from-bit-identical.sh
runner-timeout: 60
last-run: 2026-05-30T07:54:48.214827485+00:00
last-run-duration: 0.3s
---

## Description

The escape hatch — `dec init --from streams/decision-cli-
development.ttl` — must produce the same orchestration store
bytes after FT-114 ships as it did before. Operators who
hand-author streams must be able to trust that the new
auto-discover path doesn't perturb the load path they depend
on.

## Acceptance Criteria

Bash test (uses both the pre-FT-114 baseline captured as a
fixture and the post-FT-114 binary):

1. Set up a temp repo with `.product/` populated.
2. Capture a reference seeded store: place
   `tests/fixtures/init-from-ref-store.nq` (committed
   alongside this TC). This was produced by running
   `dec init --from streams/decision-cli-development.ttl` on
   the same fixture at FT-110-era binary version.
3. Run `dec init --from streams/decision-cli-
   development.ttl --yes` against the temp repo using the
   FT-114 binary.
4. Sort the resulting `.dec/store/orchestration.nq` and the
   reference. Assert byte-equal.
5. Also assert:
   - `.dec/streams/` directory either doesn't exist OR is
     empty — the `--from` path does NOT generate a new
     stream file (it uses the one passed in).
   - The console output contains `loaded from
     streams/decision-cli-development.ttl` — confirms the
     `--from` path was taken, not the auto-discover.

The bash script writes the temp repo skeleton, runs init,
sorts both `.nq` files, and `diff`s them; exits zero only on
clean diff.