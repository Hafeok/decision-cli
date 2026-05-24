---
id: TC-060
title: dec verify env new produces identical artifact via CLI and MCP
type: exit-criteria
status: passing
validates:
  features: []
  adrs: []
phase: 2
runner: cargo-test
runner-args: -p decision-cli --test tc_060_dec_verify_env_new_produces_identical_artifact_via
runner-timeout: 120
last-run: 2026-05-24T19:13:54.892973793+00:00
last-run-duration: 0.3s
---

## Description

[FT-038](FT-038)'s exit criterion: creating an env via `dec verify env new` (CLI) and `dec_verify_env_new` (MCP) with equivalent inputs produces the same on-disk Turtle and the same store projection. Validates the single-handler discipline from [ADR-029](ADR-029) in concrete form.

## Acceptance Criteria

1. **CLI happy path.** `dec verify env new --type ephemeral-tempdir --safety-class isolated --allowed-ops shell,filesystem` in a tempdir creates `.dec/verify/env/ENV-NNN-*.ttl`, exits 0, and prints the minted id + path.

2. **MCP happy path.** Invoking `dec_verify_env_new` via the MCP server with the equivalent JSON input (`{"env_type": "ephemeral-tempdir", "safety_class": "isolated", "allowed_ops": ["shell", "filesystem"]}`) returns `{ id, path }` and creates the same file structure.

3. **Byte-equal Turtle.** Running the CLI in tempdir A and the MCP variant in tempdir B with identical inputs produces canonically equal Turtle files for the minted env (modulo the `ENV-NNN` id, which is independently minted in each store).

4. **SHACL gates both surfaces.** Both forms reject a missing `--endpoint` on `--type remote-http` with `Error::SchemaViolation`; the CLI exits 1 with stderr diagnostic, the MCP returns the structured error with the same `detail`.

5. **Caller-supplied id with collision.** Invoking either surface twice with `--id ENV-007` (or `{"id": "ENV-007"}` for MCP) and different `allowed_ops` causes the second invocation to fail with `Error::DuplicateId { id: "ENV-007" }`.

6. **Remote env requires endpoint.** `--type remote-http --safety-class shared-non-destructive --allowed-ops http --endpoint https://example.com` succeeds; omitting `--endpoint` fails on both surfaces with the same error.

## Fixture

- Two tempdirs with `dec init` completed.
- A test MCP client spawning `dec mcp serve`.

## Out of scope

- env list / show (TC-061, TC-062).
- Naming convention enforcement (TC-051).