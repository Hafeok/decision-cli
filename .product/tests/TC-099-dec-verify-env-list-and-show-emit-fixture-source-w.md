---
id: TC-099
title: dec verify env list and show emit fixture_source when set
type: scenario
status: passing
validates:
  features:
  - FT-053
  adrs: []
phase: 1
runner: bash
runner-args: tests/scripts/tc-099-env-list-show-fixture-source.sh
runner-timeout: 120
last-run: 2026-05-23T18:00:08.010786381+00:00
last-run-duration: 0.3s
---

## Purpose

The list and show projections must passthrough `fixture_source` so the operator (and downstream MCP consumers) can see the fixture binding without re-parsing the on-disk Turtle. This TC pins the field's presence in both JSON and text renderings.

## Given

- A working directory with `dec init --template engineering-development` completed.
- A fixture directory `tests/fixtures/fixture-tc-099/` created.
- An env created via `dec verify env new --id ENV-FIXT-099 --type ephemeral-tempdir --safety-class isolated --allowed-ops shell,filesystem --fixture-source tests/fixtures/fixture-tc-099`.
- A second env `ENV-NOFIXT-099` created without `--fixture-source`.

## When

```bash
dec verify env list --format json
dec verify env show --id ENV-FIXT-099 --format json
dec verify env show --id ENV-NOFIXT-099 --format json
dec verify env show --id ENV-FIXT-099 --format text
dec verify env show --id ENV-NOFIXT-099 --format text
```

## Then

1. `list --format json` is a JSON array; the entry for `ENV-FIXT-099` carries `fixture_source: "tests/fixtures/fixture-tc-099"`; the entry for `ENV-NOFIXT-099` omits the `fixture_source` key entirely.
2. `show --id ENV-FIXT-099 --format json` carries `fixture_source: "tests/fixtures/fixture-tc-099"`.
3. `show --id ENV-NOFIXT-099 --format json` omits the `fixture_source` key.
4. `show --id ENV-FIXT-099 --format text` includes a `fixture:` row with the path.
5. `show --id ENV-NOFIXT-099 --format text` has no `fixture:` row.

## Notes

- Omission (rather than `null`) is the contract for both the CLI JSON and the MCP envelope; the parity TCs (TC-061/TC-062) byte-compare across surfaces.