---
id: TC-055
title: dec init seeds ephemeral-cli env idempotently
type: exit-criteria
status: passing
validates:
  features: []
  adrs: []
phase: 2
runner: cargo-test
runner-args: -p decision-cli --test tc_055_dec_init_seeds_ephemeral_cli_env_idempotently
runner-timeout: 120
last-run: 2026-05-24T19:13:53.689497560+00:00
last-run-duration: 0.3s
---

## Description

[FT-035](FT-035) extends `dec init` to seed an `ephemeral-cli` `dec:VerificationBench`. The seed must be present after a fresh init, reproducible byte-for-byte across runs, and unchanged when `dec init` is re-invoked against an already-initialised store.

## Acceptance Criteria

1. **Seed exists after init.** After `dec init --from <seed>.ttl` completes in a clean tempdir, `.dec/verify/env/BNCH-001-ephemeral-cli.ttl` exists and parses as Turtle.

2. **Seed content.** The seed env carries:
   - `dec:envType "ephemeral-tempdir"`,
   - `dec:safetyClass "isolated"`,
   - `dec:allowedOps ("shell" "filesystem" "sparql-local")` in that order,
   - non-empty `dec:setup` and `dec:teardown`.

3. **Idempotency.** Re-running `dec init --from <seed>.ttl` against the same dir does not modify `BNCH-001-ephemeral-cli.ttl` (file mtime may change, but byte content is identical) and does not produce a second env file.

4. **Reproducibility across runs.** Running `dec init` in two separate clean tempdirs produces the same `BNCH-001-ephemeral-cli.ttl` bytes (canonical Turtle serialisation).

5. **Store projection.** After init, a SPARQL query against the orchestration store returns exactly one `dec:VerificationBench` whose IRI is `https://decision-cli.dev/ns/bench/BNCH-001-ephemeral-cli`.

## Fixture

- Two clean tempdirs.
- The `decision-cli-development.ttl` stream seed.

## Out of scope

- SHACL shape correctness (TC-054).
- Loading additional envs authored after init (covered by FT-038's TC).