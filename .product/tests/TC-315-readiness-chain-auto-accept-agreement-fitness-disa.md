---
id: TC-315
title: Readiness chain auto-accept agreement fitness disagreement-rate stays under threshold
type: scenario
status: unimplemented
validates:
  features: []
  adrs:
  - ADR-075
  - ADR-014
phase: 1
runner: bash
runner-args: scripts/checks/readiness-autonomy-agreement.sh
runner-timeout: 60
observes:
- exit-code
- stdout
---

## Purpose

Cross-cutting fitness function (ADR-014 entry). ADR-075 makes auto-acceptance safe by promising that the auto-accept policy will be retired or tightened if the human-disagreement rate (later reopens, retroactive rejections, defects opened against auto-accepted artifacts) exceeds a configured threshold over a rolling window. This check is the watchdog that proves the system is operating within that bound; it reads the orchestration store, computes the disagreement rate over the window, and reports.

## Acceptance

- The script exits 0 when the disagreement rate over the configured rolling window (e.g. last 30 days, or last N auto-accepted verdicts) is strictly less than the threshold declared in ADR-075.
- The script exits 1 when the disagreement rate equals or exceeds the threshold — this is a fitness-function failure and blocks merge per ADR-014.
- The script emits structured stdout (e.g. JSON) reporting `{auto_accepted_count, disagreement_count, rate, threshold, window}` so the PR comment can surface the number.
- The script exits 0 (with an informational stdout note) when there have been zero auto-accepted verdicts in the window (no signal yet, rate is undefined but not a failure).

## Inputs

Live repo state: SPARQL queries against `.dec/store/orchestration.nq` enumerating auto-accepted verdicts (per TC-314 routing) within the rolling window, and counting downstream human-reopen or human-rejection events that target the auto-accepted artifacts. The threshold and window are read from a config file (e.g. `.product/fitness-config.toml`) or hardcoded constants in the script keyed to ADR-075's stated values.

## Out of scope

- Per-verdict routing correctness (covered by TC-314).
- Provenance shape (covered by TC-313).

