---
id: TC-380
title: cli_list_show_pairing_registry_covers_every_clap_list_subcommand
type: scenario
status: unimplemented
validates:
  features: []
  adrs:
  - ADR-081
phase: 4
runner: bash
runner-args: scripts/checks/cli-pairing-registry-coverage.sh
runner-timeout: 30
observes:
- exit-code
- stdout
---

## Description

Catches the "forgot to register a new list verb" failure mode for [ADR-081](ADR-081). The platform totality check (TC-379) is registry-driven; a new `dec <noun> list` subcommand that lands without an entry in `crates/decision-cli/src/core/cli_pairing.rs` is invisible to that check and silently re-opens the bug class the ADR was authored to close.

This TC greps the clap command tree under `crates/decision-cli/src/cli/` for `<noun> list` subcommand declarations and asserts each one is also present in the pairing registry.

## Given

- The CLI sources are present under `crates/decision-cli/src/cli/`.
- The pairing registry is at `crates/decision-cli/src/core/cli_pairing.rs`.
- Both are first-party files (no generated code involved).

## When

```bash
bash scripts/checks/cli-pairing-registry-coverage.sh
```

## Then

1. Script exits 0 when every `<noun> list` subcommand discoverable in the clap tree has a matching `(noun, list_verb, ...)` row in `cli_pairing.rs`.
2. Script exits 1 when any clap-declared list verb is absent from the registry. Stdout names the missing noun and the file:line where the clap subcommand is declared.
3. Registry entries that point at non-existent clap subcommands are also flagged (the registry must not outpace the clap tree either).

## Notes

- The script is a syntactic check, not a behavioural one — it does not invoke the CLI. Cheap to run; safe to include in the platform suite.
- A future refactor that auto-derives the registry from the clap tree (via build script or proc-macro) would retire this TC; until then, the registry is hand-maintained and the TC is its safety net.
