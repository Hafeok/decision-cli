---
id: TC-379
title: cli_list_show_totality_every_listed_iri_resolves_via_show
type: scenario
status: unimplemented
validates:
  features: []
  adrs:
  - ADR-081
phase: 4
runner: bash
runner-args: scripts/checks/cli-list-show-totality.sh
runner-timeout: 60
observes:
- exit-code
- stdout
- stderr
---

## Description

Platform-level fitness function for [ADR-081](ADR-081). Walks the `(noun, list_verb, show_verb, log_verb_opt)` registry declared in `crates/decision-cli/src/core/cli_pairing.rs` and, for each tuple, asserts that every IRI returned by `dec <noun> list --limit 50 --format json` resolves cleanly via `dec <noun> show <iri>` (and `dec <noun> log <iri>` where the log verb is present).

The TC reaches `product verify --platform` via the cross-cutting ADR linkage and runs on every PR.

## Given

- A working directory initialised via `dec init` (FT-008).
- The orchestration store has whatever sessions / events / feedback / verify graphs / etc. happen to exist in it — the TC asserts a universal property over the *currently extant* IRIs, not over a fixture set.
- The CLI pairing registry at `crates/decision-cli/src/core/cli_pairing.rs` declares at minimum the `session` noun (list, show, log). Additional nouns are added to the registry as their list/show pairs land.
- `dec <noun> list --format json` emits structured output (one object per row with at least an `iri` field). If the JSON path is not yet shipped on a given noun, the script falls back to extracting the trailing `iri=...` token from the human-formatted output.

## When

```bash
bash scripts/checks/cli-list-show-totality.sh
```

## Then

1. Script exits 0 — every IRI returned by every registered list verb resolves cleanly through the paired show verb (and log verb, where applicable).
2. Empty stores pass trivially — when a noun's list returns zero rows, the property is vacuously true for that noun.
3. On failure (exit 1), stdout enumerates each `(noun, iri, verb, exit_code)` quadruple that violated the invariant, plus a stderr excerpt for the first failure per noun. The operator can map each line back to the producer code path without grepping.

## Notes

- The two-tier exit-code contract from [ADR-013](ADR-013) applies: exit 0 = clean (including empty registries), exit 1 = at least one violation. No warn-band — list↔show consistency is binary.
- The TC depends on the canonical projection refactor (each noun owns a single `core::graph::<noun>::project` function used by both list and show) being in place for every registered noun. Until the refactor lands per-noun, the TC catches drift on whichever nouns *have* been refactored; nouns still on the old "two divergent SPARQL bodies" pattern remain susceptible to silent bugs of the kind described in ADR-081's Context.
- Pair with TC-380 (registry coverage): the platform totality check is only as comprehensive as the registry, so TC-380 prevents new list verbs from landing without a registry entry.
