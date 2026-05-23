---
id: TC-096
title: dec_verify_env_list_resilient_to_corrupt_entries
type: scenario
status: passing
validates:
  features:
  - FT-039
  adrs: []
phase: 1
runner: bash
runner-args: tests/scripts/tc-096-env-list-corrupt-resilience.sh
runner-timeout: 120
last-run: 2026-05-23T17:59:47.437016574+00:00
last-run-duration: 0.3s
---

## Purpose

Discovered alongside TC-094 / TC-095 while dogfooding `dec verify` on decision-cli's own implementation (2026-05-22). When a single VerificationEnvironment in the orchestration store is malformed (e.g. two `dec:allowedOps` heads from the silent-append bug in TC-094, or a missing required field from a future schema drift), `dec verify env list` currently aborts the entire listing with `env "...ENV-NNN" declares N dec:allowedOps heads`. The operator cannot see the other (healthy) envs, cannot triage which env is broken without grepping the store directly, and cannot use list output to drive a recovery workflow.

This is a defence-in-depth concern: even after TC-094 lands a proper upsert/refuse contract, corruption can still arise from manual store edits, partial migrations, or future write-path bugs. The list command should be the operator's first lifeline, not the first thing to fall over.

The contract `dec verify env list` should hold: **the listing must complete for every env that is parseable, and report a structured per-row error for envs that are not** — instead of aborting the whole command on the first malformed entry.

Related: TC-094 (the upstream upsert bug that produces this state today), FT-039 (this feature).

## Given

- A `.dec/` initialized via `dec init --template engineering-development`.
- Three environments in the store:
  - `ENV-HEALTHY-A` — well-formed.
  - `ENV-HEALTHY-B` — well-formed.
  - `ENV-BROKEN` — corrupt (two `dec:allowedOps` heads, simulating the TC-094 condition; achievable in the test by direct store seed or by running the TC-094 reproducer first).

## When

```bash
dec verify env list
```

(or its `--format json` equivalent)

## Then

1. The command exits 0 (or with a documented non-zero "partial success" code, e.g. exit 2) — **not** an internal-error exit.
2. Output includes rows for `ENV-HEALTHY-A` and `ENV-HEALTHY-B` with their full details.
3. Output includes a row for `ENV-BROKEN` with a structured error marker (e.g. `<corrupt: 2 dec:allowedOps heads>`) in place of the offending field — enough for the operator to identify the env id and the failure mode.
4. JSON output (`--format json`) places the error in a typed field (e.g. `"error": { "kind": "MultipleAllowedOpsHeads", "count": 2 }`) on that env's record, not as a top-level error replacing the whole payload.
5. Stderr remains usable for non-fatal warnings; the offending env id appears there once, with a pointer to a recovery command if/when one exists.

## Notes

- This is intentionally narrower than "validate every env on every list" — full schema validation belongs in a separate `dec verify env check` command. List should only catch errors that prevent it from rendering a row.
- The same principle should be applied to `dec verify graph list` (FT-042) and `dec verify env show` (FT-040) — but show is per-id and may legitimately exit non-zero on a single corrupt env. Scope this TC to list only; file follow-ups for the others if/when reproduced.
- If the schema check is centralised (one parser that returns `Result<Env, EnvError>`), this becomes a one-line change at the list iterator. Worth confirming during the fix.