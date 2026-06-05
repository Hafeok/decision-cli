---
id: TC-381
title: dec_session_list_returns_rows_with_all_four_columns_populated
type: scenario
status: unimplemented
validates:
  features:
  - FT-012
  adrs: []
phase: 4
runner: bash
runner-args: tests/scripts/tc-381-session-list-smoke.sh
runner-timeout: 60
observes:
- exit-code
- stdout
---

## Description

Smoke test for `dec session list` against a populated orchestration store. Asserts the verb returns at least one row, exits 0, and that each row carries the four documented columns: `started_at`, `feature_id`, `status`, `iri`.

Companion TC to TC-382. This one asserts that list emits rows at all (and in a parseable shape); TC-382 asserts that the IRIs in those rows resolve through `show` and `log`.

## Given

- A working directory initialised via `dec init` (FT-008).
- The orchestration store has been populated with at least one `dec:Session` resource by some upstream test step (typically: run `dec implement FT-XXX` against a fixture feature, or load a pre-seeded `.dec/store/orchestration.nq`).

## When

```bash
dec session list --limit 20
```

## Then

1. Command exits 0.
2. Stdout contains at least one session row (not the `(no sessions)` placeholder).
3. Every row matches the documented format: `<started_at>  feature=<feature_id>  status=<status>  iri=<iri>` — placeholder values `(unknown-time)`, `(no-feature)`, `(pending)` are accepted in their respective slots; missing column tokens are a failure.
4. The IRIs printed in the `iri=` slot are well-formed (`urn:` or `https://` scheme).

## Notes

- The format assertion is deliberately weak (placeholders allowed) so the TC tolerates partial-shape sessions — those are exactly the sessions that motivated [ADR-081](ADR-081). TC-382 then enforces the stronger "and show must resolve them" property.
- If the orchestration store is empty when the TC runs, the test runner should fail it with a clear `unrunnable: store empty` rather than silently passing on `(no sessions)`. The runner script is responsible for seeding or aborting.
