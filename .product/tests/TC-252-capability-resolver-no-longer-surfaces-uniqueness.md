---
id: TC-252
title: Capability resolver no longer surfaces uniqueness errors after bootstrap with FT-118 lands
type: scenario
status: passing
validates:
  features:
  - FT-118
  adrs: []
observes:
- exit-code
phase: 4
runner: bash
runner-args: tests/scripts/tc-252-resolver-no-uniqueness-after-bootstrap.sh
runner-timeout: 60
last-run: 2026-05-29T19:57:40.853088466+00:00
last-run-duration: 0.0s
---

## Description

End-to-end regression: reproduce the FT-114-drive-block we
just hit. Compose a fresh workdir, run `dec init` (seeds v1
binding), then run `bootstrap_catalog.py --migrate` (was
silently adding v7 alongside v1). With FT-118 in, the
resolver should resolve cleanly; the bootstrap should
report deactivations.

## Acceptance Criteria

Bash test:

1. Build a temp workdir; run `dec init --template
   engineering-development`. Assert exit 0. The init seeds
   `verify-graph-author/v1` binding active.
2. Run `python3 scripts/bootstrap_catalog.py --graph-path
   <temp> --migrate`. Assert exit 0.
3. Assert the bootstrap stdout contains a "deactivated"
   line naming `verify-graph-author/v1`.
4. Run `dec verify graph generate FT-X --bench <some
   bench>` against the temp workdir (FT-X has at least
   one TC). Assert it does NOT fail with the substring
   "uniqueness invariant violated".
5. Assert the resolver successfully bound to the v7
   capability (i.e., the model_identifier used was
   gpt-oss-120b or whichever the YAML names — surfaced
   via the worker bundle's `model_id` field).

This is the canonical regression test against the bug
that blocked FT-114's drive on 2026-05-29.