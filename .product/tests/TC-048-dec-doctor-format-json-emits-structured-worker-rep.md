---
id: TC-048
title: dec doctor format json emits structured worker report
type: exit-criteria
status: passing
validates:
  features:
  - FT-016
  adrs: []
phase: 1
runner: bash
runner-args: tests/scripts/tc-048-dec-doctor-json.sh
runner-timeout: 60
last-run: 2026-05-20T08:03:50.124890079+00:00
last-run-duration: 0.3s
---

## Description

`dec doctor --format json` exists so that CI and scripting layers can consume the audit without scraping the text format. The JSON document is the canonical structured form of the same audit — its exit-code contract matches the text mode, and its schema is stable.

## Acceptance Criteria

Given an already-initialised `.dec/store/orchestration.nq`:

1. **Exit code parity.** `dec doctor --format json` exits with the same status as `dec doctor` (text mode) under identical environment, both for the all-resolved and any-missing cases.

2. **Single JSON document.** stdout parses as exactly one JSON document — no shell-level prefix, no trailing text, no ANSI escapes.

3. **Workers array.** The document has a top-level `workers: [...]` array. Each entry has:
   - `role: String` — e.g. `"code-writer"`.
   - `status: "ok" | "missing" | "inactive"`.
   - `resolved_via: "override" | "env" | "path" | "sibling-workspace" | "python-module" | null` — null exactly when `status` is `missing` or `inactive`.
   - `resolved_command: [String]` — non-empty argv when `status == "ok"`, absent or empty otherwise.

4. **Install hints on missing rows.** Entries with `status == "missing"` have an `install_hints: [String]` array containing at least one suggestion that mentions the manifest's `source_hint`.

5. **Summary block.** A top-level `summary` object reports `{ ok: Integer, missing: Integer, inactive: Integer }` and these counts equal the partition of the `workers` array.

6. **Manifest fingerprint.** A top-level `manifest_sha256: String` matches the sha256 of `dec`'s embedded worker manifest.

7. **Diagnostics.** When a probe fails (e.g. `python3 -c "import X"` returns non-zero), the captured stderr appears in `workers[].diagnostics: String[]` and never propagates as a `dec` error.

## Fixture

- Same tempdir as TC-046/TC-047.
- Assert via a JSON parser, not regex, so schema breakage is detected even when output looks superficially correct.

## Out of scope

- Stability of the JSON schema across major `dec` versions (separate concern).
- Pretty-printing or key ordering — consumers must not depend on either.