---
id: TC-168
title: Bundle assembler populates the five ADR-066 fields from catalog artifacts via SPARQL CONSTRUCT
type: exit-criteria
status: failing
validates:
  features:
  - FT-102
  adrs: []
phase: 1
runner: cargo-test
runner-args: tc_168_bundle_assembler_populates_the_five_adr_066_fields
runner-timeout: 120
last-run: 2026-05-28T08:49:05.201345116+00:00
last-run-duration: 1.1s
failure-message: "warning: function `handler_internal` is never used\n  --> crates/decision-cli/src/features/loop_inspect/mod.rs:85:4\n   |\n85 | fn handler_internal(detail: String) -> HandlerError {\n   |    ^^^^^^^^^^^^^^^^\n   |\n   = note: `#[warn(dead_code)]` (part of `#[warn(unused)]`) on by default\n\nwarning: missing documentation for a variant\n  --> crates/decision-cli/src/core/dispatch_session.rs:37:5\n   |\n37 |     Completed,\n   |     ^^^^^^^^^\n   |\n   = note: requested on the command line with `-W missing-docs"
---

## Claim

Invoking `dec verify graph generate <FT> --environment <ENV>` causes the bundle assembler to populate all five ADR-066 fields on the `VerifyGraphAuthorInput` payload by SPARQL-querying the catalog artifacts FT-101 ships. The bundle's `bundle_hash` covers all five fields; their content-hashes are recorded on the bundle metadata for replay determinism.

## Scenarios

### Setup

- Fresh `.dec/` initialised via `dec init`.
- Seed catalog:
  - `CR-001` covering `dec verify graph new`, `CR-002` covering `dec verify graph run`, `CR-003` covering `dec sparql query` — all `dec:capabilityVersion = "0.3.0"`.
  - `OD-001` with namespace `https://decision-cli.dev/ns#` and classes `VerificationGraph`, `VerificationStep`, `Session` plus their canonical predicates.
  - `EX-001`, `EX-002` exemplars both `dec:appliesToSafetyClass = "isolated"`, both with backing approved VGRs.
- Seed env `ENV-001` with `safetyClass = "isolated"`, `envType = "ephemeral-tempdir"`, and a `dec:concreteCapabilities` block declaring binaries `[dec, bash]`, writable paths `[$DEC_VERIFY_TMP, ./]`, env vars `[DEC_VERIFY_TMP, PATH]`.
- Seed feature `FT-FIXTURE` with TCs `[TC-FX-A, TC-FX-B]`.

### Scenario A — full enrichment when catalog is populated

Invoke `dec verify graph generate FT-FIXTURE --environment ENV-001 --print-only --format json` (the `--print-only` flag prints the assembled bundle and the proposal without persisting; the test asserts on the bundle structure, not on the worker output). Assertions on the bundle JSON:

- `cli_surface.commands` contains exactly 3 entries with `command` values matching `dec verify graph new`, `dec verify graph run`, `dec sparql query`.
- `cli_surface.capability_version` is `"0.3.0"`, matching the running `dec --version`.
- `ontology_vocabulary.namespace == "https://decision-cli.dev/ns#"` and `ontology_vocabulary.classes` lists `VerificationGraph`, `VerificationStep`, `Session`.
- `store_query_surface.kind == "local-oxigraph"`, `store_query_surface.query_command == "dec sparql query --store"` (or whatever literal the env-type table declares; the test pins the value).
- `env_capabilities.binaries_on_path == ["dec", "bash"]`, `env_capabilities.writable_paths == ["$DEC_VERIFY_TMP", "./"]`.
- `exemplar_graphs` has length 2 — both EX-001 and EX-002 — and each entry carries the exemplar's `pattern_name`, `rationale`, and a reference to the underlying VG.
- The bundle's metadata block contains `catalog_hashes` listing the content-hash of each `CR-*`, `OD-*`, `EX-*` artifact that was pulled. Replaying the same query produces the same `bundle_hash`.

### Scenario B — env without `dec:concreteCapabilities` falls back to env-type default

Replace `ENV-001` with `ENV-002` identical except no `dec:concreteCapabilities` block. Invoke the same generate verb. Assertions:

- `env_capabilities` is populated from the env-type default table (the assembler's per-type fallback for `ephemeral-tempdir`).
- The bundle's metadata block carries a `warnings: ["env ENV-002 has no dec:concreteCapabilities block; using env-type default for ephemeral-tempdir"]` entry.
- The bundle still includes the other four fields fully populated.

### Scenario C — query templates are themselves artifacts

Inspect `.dec/catalog/` for `dec:QueryTemplate` artifacts named `assembler-cli-surface`, `assembler-ontology`, `assembler-exemplars` (per the FT-102 spec). Assertions:

- Each query template exists on disk and is a SHACL-valid `dec:QueryTemplate`.
- Replacing one of the templates (e.g. tightening the `cli_surface` query to filter by an additional predicate) and re-running the generate verb produces a bundle reflecting the new query — proves the assembler reads the template, not a hardcoded query.

### Scenario D — replay determinism

Capture the bundle from Scenario A's run as a JSON file. Replay the assembler with `--from-recorded-catalog-hashes <file>` (a `dec verify graph generate` flag that pins the catalog snapshot to the recorded hashes, ignoring current catalog state). After mutating the catalog (e.g. adding `CR-004`), replay. Assertions:

- The replayed bundle exactly matches the original (same hash, same five fields).
- Without the flag, a fresh generate would include CR-004 in `cli_surface` — confirming the replay flag pinned the catalog snapshot.

## Runner

`bash tests/scripts/tc-168-bundle-enrichment.sh`. Temp `.dec/`, fixtures for the catalog artifacts ship alongside. The test does **not** invoke the LLM worker — `--print-only` returns after assembly but before subprocess dispatch (or the test uses `VERIFIER_STUB` / equivalent to short-circuit the worker). Asserting on the bundle JSON is the entire test surface.

## Non-goals

- The worker's prompt-side consumption of the bundle (covered by TC-169, TC-170).
- The validator's behaviour (TC-169).
- LLM behaviour against the enriched bundle (out of slice; the smoke-test of regenerating FT-097..FT-100 lives in the FT-102 description as a follow-up).