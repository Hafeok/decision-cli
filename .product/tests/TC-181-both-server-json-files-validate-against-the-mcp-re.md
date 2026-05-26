---
id: TC-181
title: Both server.json files validate against the MCP registry schema on every PR
type: scenario
status: unimplemented
validates:
  features:
  - FT-106
  adrs: []
phase: 1
---

## Claim

Both `crates/product-cli/server.json` and `crates/decision-cli/server.json` (or wherever the dec MCP server's manifest is committed) validate against the MCP registry's published `server.schema.json` on every PR. Schema drift fails CI; the test catches it before a release attempt finds out.

## Scenarios

### Setup

- The committed schema file at a known path (e.g. `tests/fixtures/server.schema.json`, mirroring product-cli's existing convention of shipping the schema for offline validation).
- A JSON schema validator available in the CI environment (`jsonschema` Python package, `ajv` node, or a Rust crate — implementation detail).

### Scenario A — both server.json files validate

Run the validator with the schema against each committed `server.json`. Assertions:
- Exit code: 0 for both files.
- No validation errors reported.
- The validator output includes the file path and the schema version it validated against (for audit).

### Scenario B — required fields are present

Beyond schema validation, assert presence of business-required fields (the registry rejects on these too, but better-fast-CI-feedback):
- `$schema` URL matches the expected version (e.g. `2025-09-29`).
- `name` matches the expected pattern `io.github.<owner>/<repo>` and is unique across both files (`product-cli` vs `decision-cli`).
- `description` is non-empty and ≥ 16 chars (registry minimum).
- `version` exists (placeholder allowed pre-publish; the publish workflow substitutes).
- `repository.url`, `repository.source`, `repository.id` all present.
- `packages` array has ≥ 1 entry.
- Each `packages[*]` entry has `registryType`, `identifier`, `version`, `transport`, `fileSha256` (placeholder allowed), and `runtimeArguments`.

### Scenario C — schema is up-to-date

A separate sub-check: download the current registry schema (`https://static.modelcontextprotocol.io/schemas/2025-09-29/server.schema.json` or whatever the `$schema` URL points to) and compare to the committed copy in `tests/fixtures/server.schema.json`. Assertions:
- If they match, all good — pass.
- If they differ, the test fails with a warning that the committed schema is stale; the operator updates it.

This sub-check runs nightly, not on every PR (network dependency); PR runs use the committed copy.

### Scenario D — deliberate breakage caught

Mutate one of the `server.json` files locally to introduce a known-bad change (e.g. remove the `name` field). Run the test. Assertions:
- Exit code: 1.
- Stderr names the failing file and the missing field.
- The test reverts the mutation after asserting (so it doesn't leak state).

This sub-check is for the test-of-the-test pattern — proving the validator actually catches violations.

### Scenario E — placeholder values are tolerated pre-publish

The committed `server.json` files carry placeholders for `version`, `fileSha256`, and parts of `identifier`. The validator must accept these placeholders (they're substituted at publish time):
- `version`: `"<PLACEHOLDER>"` or a SemVer string (both pass).
- `fileSha256`: 64 hex characters (the placeholder is 64 zeros; valid hex; passes the schema's `pattern` constraint).
- `identifier`: URL template with `v<VERSION>` substring; the schema accepts any valid URL.

### Scenario F — version pin matches the workspace

A sub-assertion: the `version` field in each `server.json` matches the workspace's current version (read from `Cargo.toml`). Drift here means the operator bumped the workspace version without updating the manifests; the publish workflow will fix it at publish time, but committing a matched value reduces ambiguity for readers. This is a soft assertion (warning, not failure) — the publish workflow has the final say.

## Runner

`bash tests/scripts/tc-181-server-json-schema.sh`. Fast, no network on the per-PR path; integrates as a standard CI check. The Scenario C network sub-check runs on a separate nightly workflow.

## Non-goals

- Validating the registry's behaviour when it receives the manifest (TC-180 covers integration).
- Auto-generating server.json from `Cargo.toml` (out of scope — manual + placeholder substitution is the pattern product-cli already established).
- Schema versioning policy (the operator pins the schema URL; bumps are deliberate).
- Validating other registry-listed servers (only ours; the test is per-workspace).
