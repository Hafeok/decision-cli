---
id: TC-056
title: Each VerificationStep kind SHACL shape validates its required fields
type: exit-criteria
status: passing
validates:
  features: []
  adrs: []
phase: 2
runner: cargo-test
runner-args: -p decision-cli --test tc_056_each_verificationstep_kind_shacl_shape_validates_i
runner-timeout: 120
last-run: 2026-05-23T17:59:44.958686025+00:00
last-run-duration: 0.2s
---

## Description

[FT-036](FT-036) defines six seed step kinds, each with its own SHACL shape and `dec:requiredOps` declaration per [ADR-028](ADR-028). This TC exercises the per-kind shape selection: the right shape fires based on `dec:stepType`, and each kind's required fields are enforced.

## Acceptance Criteria

For each step kind, a well-formed instance commits and a malformed instance fails:

1. **shell-command.** Pass: `dec:stepType "shell-command"; dec:command "ls"; dec:expectExitCode 0`. Fail: missing `dec:command`.

2. **sparql-assertion.** Pass: `dec:stepType "sparql-assertion"; dec:target ".dec/store"; dec:query "SELECT ?s WHERE { ?s ?p ?o }"; dec:expectRows 1`. Fail: missing `dec:query`.

3. **file-assertion.** Pass: `dec:stepType "file-assertion"; dec:path ".dec/store/orchestration.nq"`. Fail: missing `dec:path`.

4. **http-request.** Pass: `dec:stepType "http-request"; dec:method "GET"; dec:url "https://example.com"; dec:expectStatus 200`. Fail: missing `dec:url`.

5. **wait-for.** Pass: `dec:stepType "wait-for"; dec:condition <ref>; dec:timeout "PT10S"`. Fail: missing `dec:timeout`.

6. **capture.** Pass: `dec:stepType "capture"; dec:bindAs "manifest_sha"`. Fail: missing `dec:bindAs`.

7. **Unknown stepType.** A step with `dec:stepType "rocketship"` fails with `Error::UnknownStepKind { value: "rocketship" }`.

8. **`${name}` reservation.** A `shell-command` step whose `dec:command` contains `dec init ${prior_stream}` commits successfully — the placeholder is preserved verbatim in storage and no interpretation happens. The on-disk Turtle round-trips the literal string.

## Fixture

- A `StreamWriter` harness, one fragment per kind, asserting commit success or specific `SchemaViolation` detail.

## Out of scope

- Safety enforcement (TC-058, TC-059 — FT-037).
- Round-trip with ordered step lists (TC-057).