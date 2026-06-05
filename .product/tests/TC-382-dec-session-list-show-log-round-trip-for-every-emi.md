---
id: TC-382
title: dec_session_list_show_log_round_trip_for_every_emitted_iri
type: scenario
status: unimplemented
validates:
  features:
  - FT-012
  adrs: []
phase: 4
runner: bash
runner-args: tests/scripts/tc-382-session-list-show-log-round-trip.sh
runner-timeout: 120
observes:
- exit-code
- stdout
- stderr
---

## Description

The session-noun specialisation of [ADR-081](ADR-081)'s totality invariant, scoped to [FT-012](FT-012)'s `dec session list / show / log` surface. For every IRI returned by `dec session list`, both `dec session show <iri>` and `dec session log <iri>` must exit 0.

This is the load-bearing TC that would have caught the failure described in ADR-081's Context: verify-graph-runner sessions appearing in list but rejected by show because `show`'s SPARQL hardcodes `prov:used ?bundle . prov:used ?model` as required patterns that those sessions don't satisfy.

## Given

- A working directory initialised via `dec init` (FT-008).
- The orchestration store contains sessions from at least two distinct producer code paths — e.g. an implementer session (`dec implement FT-XXX`) plus a verify-graph-runner session ([FT-098](FT-098)). The runner script either seeds these or aborts with `unrunnable: insufficient session diversity`.

## When

```bash
bash tests/scripts/tc-382-session-list-show-log-round-trip.sh
```

The script:

1. Invokes `dec session list --limit 50` and extracts every IRI from the output.
2. For each IRI, invokes `dec session show <iri>` and asserts exit 0.
3. For each IRI, invokes `dec session log <iri>` and asserts exit 0.

## Then

1. The script exits 0 — every listed IRI resolves through both show and log.
2. On failure (exit 1), stdout names each `(iri, verb, exit_code)` triple that violated the invariant, and stderr captures the first failure's error message verbatim so the operator can map back to the SPARQL query that rejected the IRI.

## Notes

- This TC overlaps with TC-379 by construction. The redundancy is deliberate: TC-379 is the cross-cutting check that runs through `product verify --platform`; TC-382 is the per-feature check that runs through `product verify FT-012`. Either gate alone would catch the bug, but having both means a developer running `product verify FT-012` after touching the session code path gets immediate feedback without needing to remember to run the platform suite separately.
- Once the canonical-projection refactor in `core::graph::session::project` lands, this TC and TC-381 should both stay green by construction — list and show share the same projection clause, so set membership is identical.
