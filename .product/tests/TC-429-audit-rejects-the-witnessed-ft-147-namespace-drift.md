---
id: TC-429
title: Audit rejects the witnessed FT-147 namespace drift — canonical_namespace fires on decisionframework.com
type: invariant
status: passing
validates:
  features:
  - FT-172
  adrs:
  - ADR-080
  - ADR-013
phase: 1
runner: bash
runner-args: scripts/checks/tc-429-audit-namespace-negative.sh
runner-timeout: 60
observes:
- exit-code
- stdout
last-run: 2026-06-11T18:18:31.674452582+00:00
last-run-duration: 0.0s
---

## Purpose

FT-172: the canonical-namespace check catches exactly the defect the pre-FT-172 audit passed on FT-147 — a worker-invented IRI base (`decisionframework.com`) in the vocab cell. The committed FT-147 sandbox is the witnessed fixture.

## Mechanism

`scripts/checks/tc-429-audit-namespace-negative.sh` runs the audit against `.dec/cluster/FT-147` with the cell list and asserts exit-code 1 with `check=canonical_namespace` on stdout/stderr naming the offending file.

## Pass criteria

Observed surfaces: exit-code and stdout. Exit-code 0 — the audit refused the witnessed sandbox for the right reason.

## Fail criteria

Exit-code 1 — the audit passed the bad namespace or failed on a different check.