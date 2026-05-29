---
id: TC-209
title: ENV-prefix IRIs are absent from all source files after rename
type: scenario
status: unimplemented
validates:
  features:
  - FT-112
  adrs: []
observes:
- stdout
- exit-code
phase: 4
runner: bash
runner-args: tests/scripts/tc-209-env-iri-absent.sh
runner-timeout: 30
---

## Description

Catches stragglers: IRI strings that weren't covered by the
identifier rename (raw `"https://decision-cli.dev/ns/env/"`
literals in tests, comments, or freshly authored code that the
identifier pass missed). The vocab module enforces consistency
on the typed constants; this TC enforces consistency on the
string literals themselves.

## Acceptance Criteria

Bash test that runs `grep -RnE '"https://decision-cli\.dev/ns/env/"|"https://decision-cli\.dev/ns#envType"|"https://decision-cli\.dev/ns#VerificationEnvironment"|"https://decision-cli\.dev/ns#ranInEnvironment"|"https://decision-cli\.dev/ns/graph/verify-env"'` over `crates/`, `tests/`, `.product/`, `docs/`, `scripts/`, and asserts zero matches.

Two specific exclusions are documented in the test script
header:
- `IRI_DEC_LEDGER_ENVIRONMENT` and `ledgerEnvironment` literals
  in `auto_dispatch.rs` — ledger axis, not renamed by FT-112.
- The migration-tool source itself (the SPARQL UPDATE in
  `_migrate-env-to-bench` necessarily mentions both old and new
  IRIs; the script grep filters out that specific file).
