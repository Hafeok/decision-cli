---
id: TC-171
title: Bundle assembler returns CatalogIncomplete error when a mandatory field has zero artifacts and no default
type: scenario
status: failing
validates:
  features:
  - FT-102
  adrs: []
phase: 1
runner: cargo-test
runner-args: tc_171_bundle_assembler_returns_catalogincomplete_error_w
runner-timeout: 120
last-run: 2026-05-28T08:49:05.201345116+00:00
last-run-duration: 0.9s
failure-message: "warning: function `handler_internal` is never used\n  --> crates/decision-cli/src/features/loop_inspect/mod.rs:85:4\n   |\n85 | fn handler_internal(detail: String) -> HandlerError {\n   |    ^^^^^^^^^^^^^^^^\n   |\n   = note: `#[warn(dead_code)]` (part of `#[warn(unused)]`) on by default\n\nwarning: missing documentation for a variant\n  --> crates/decision-cli/src/core/dispatch_session.rs:37:5\n   |\n37 |     Completed,\n   |     ^^^^^^^^^\n   |\n   = note: requested on the command line with `-W missing-docs"
---

## Claim

When the bundle assembler runs but finds zero artifacts for a mandatory field (`cli_surface`, `ontology_vocabulary`) **and** the field has no env-type default fallback, the dispatch fails with `Error::CatalogIncomplete` **before** the worker subprocess is invoked, and emits a `dec:Feedback` with `class = "gap"` pointing at the missing catalog category.

## Scenarios

### Setup

- Temp `.dec/` initialised; `.dec/catalog/` exists but is intentionally **empty** (no CR, no OD, no EX).
- Env `ENV-001` seeded normally (env_capabilities has a default fallback per env type; not in scope for this TC).
- Feature `FT-FIXTURE` with TCs.

### Scenario A — empty cli_surface fails before worker dispatch

Invoke `dec verify graph generate FT-FIXTURE --environment ENV-001`. Assertions:

- Exit code: 1.
- Stderr contains `CatalogIncomplete` and lists `cli_surface` in `missing_fields`.
- The worker subprocess was **not** invoked. Asserted by checking that no `dec:Session` with role `verify-graph-author` was created.
- One `dec:Feedback` artifact is emitted with `dec:class = "gap"` and `dec:target` resolving to `<catalog/capabilities>` (the category, since no CR exists yet).

### Scenario B — empty ontology_vocabulary same path

Seed one CR (so `cli_surface` is non-empty), but no OD. Invoke generate. Assertions:

- `Error::CatalogIncomplete { missing_fields: ["ontology_vocabulary"] }`.
- Worker not invoked. Feedback emitted targeting `<catalog/ontology>`.

### Scenario C — multiple missing fields are batched

Empty catalog. Invoke generate. Assertions:

- Single `Error::CatalogIncomplete { missing_fields: ["cli_surface", "ontology_vocabulary"] }` (both missing reported in one error, not two consecutive errors).
- One Feedback per missing category — so two Feedback artifacts in total.

### Scenario D — empty exemplars is **not** an error (warning only)

Seed CR-001 and OD-001 but no exemplars. Invoke generate. Assertions:

- Exit code: 0 — the dispatch proceeds.
- The bundle's `exemplar_graphs` field is an empty array.
- The bundle's metadata block carries a `warnings: ["no exemplar graphs found for safety_class isolated"]` entry.
- The worker is invoked (the test must assert at least the worker subprocess attempt, even if the LLM-injection seam short-circuits the actual call) and the dispatch completes normally.

This scenario pins the contract: only `cli_surface` and `ontology_vocabulary` are **mandatory**; `exemplar_graphs` is **advisory** (the prompt can still produce a graph without templates, just with less guidance).

### Scenario E — env without concreteCapabilities does not trigger CatalogIncomplete

Use an env without a `dec:concreteCapabilities` block (per TC-168 Scenario B). Invoke generate. Assertions:

- Exit code: 0.
- Bundle's `env_capabilities` is the env-type default.
- Bundle metadata carries the `warning` about missing concreteCapabilities (per TC-168 Scenario B).
- The fallback prevents this from becoming a `CatalogIncomplete` error.

### Scenario F — CapabilityVersionMismatch is a sibling error

Seed CR-001 with `dec:capabilityVersion = "0.2.0"` only; running `dec` is at `0.3.0`. Invoke generate. Assertions:

- Exit code: 1.
- Error type is `CapabilityVersionMismatch { dec_version: "0.3.0", available: ["0.2.0"] }` — distinct from `CatalogIncomplete` so the operator's remediation is different (upgrade dec, or author a new CR for 0.3.0).
- A Feedback is emitted suggesting `dec catalog capability new` for the current version.

## Runner

`bash tests/scripts/tc-171-catalog-incomplete.sh`. Temp `.dec/`. The test deliberately seeds an empty / partial catalog and asserts the assembler's pre-dispatch checks. No LLM invocation; the test exits before the worker subprocess would run.

## Non-goals

- The worker's behaviour on an empty exemplar field (the worker can return a `Gap` proposal; that's its own concern, not this validator's).
- Default env-type table contents (an implementation detail covered by code-level unit tests in the assembler).
- Cross-stream catalog overlays (out of slice).