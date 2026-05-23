---
id: TC-097
title: dec verify env new accepts --fixture-source and persists the predicate
type: scenario
status: passing
validates:
  features:
  - FT-053
  adrs: []
phase: 1
runner: bash
runner-args: tests/scripts/tc-097-env-new-fixture-source-accepts.sh
runner-timeout: 120
last-run: 2026-05-23T18:00:08.010786381+00:00
last-run-duration: 0.3s
---

## Purpose

FT-053 / ADR-032 introduces the `dec:fixtureSource` predicate on `dec:VerificationEnvironment` so verification flows with rich preconditions (`dec implement`, future `dec drive`) can declare a fixture tree to be materialised before steps execute.

This TC asserts the **authoring surface** lands the predicate on disk and in the store. It does not exercise the runner contract (steps 1–6 of ADR-032's runner contract) — that belongs to whichever future feature ships the step executor.

## Given

- A working directory with `dec init --template engineering-development` completed.
- A fixture directory `tests/fixtures/fixture-tc-097/` created under the workdir.

## When

```bash
dec verify env new \
  --id ENV-FIXT-097 \
  --type ephemeral-tempdir \
  --safety-class isolated \
  --allowed-ops shell,filesystem \
  --fixture-source tests/fixtures/fixture-tc-097
```

## Then

1. The command exits 0.
2. `.dec/verify/env/ENV-FIXT-097.ttl` contains the line `dec:fixtureSource "tests/fixtures/fixture-tc-097" ;`.
3. `dec verify env show --id ENV-FIXT-097 --format json` returns a `fixture_source` field equal to `"tests/fixtures/fixture-tc-097"`.
4. `dec verify env list --format json` includes `fixture_source` on the matching row.

## Notes

- Path validation only enforces the **authoring-time** invariants from FT-053 (relative, no `..`, exists as a directory). The runner-time materialisation is out of scope for this feature.
- The fixture directory's *content* is not validated; any tree is permitted.