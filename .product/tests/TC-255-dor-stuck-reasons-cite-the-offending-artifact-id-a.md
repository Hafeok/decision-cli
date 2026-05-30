---
id: TC-255
title: DoR Stuck reasons cite the offending artifact id and surface verbatim through the driver
type: scenario
status: unimplemented
validates:
  features:
  - FT-119
  adrs: []
phase: 1
observes:
- stdout
- exit-code
---

## Claim

Every `Stuck` reason emitted by `FeatureReadyPlanner` carries the *identity*
of the artifact(s) responsible for the gap, so the operator can open the
right file without further investigation. The driver preserves the reason
verbatim through `drive::run`, and the CLI prints it prefixed with
`FT-XXX def-ready:`.

## Scenarios

### Stuck-reason identity table

For each gap type, the reason string must contain at least the listed token(s).

| Gap | Reason must contain |
|---|---|
| preflight cross-cutting | `"ADR-"` (at least one id) + the literal `"unacknowledged"` |
| preflight domain | the domain name (e.g. `"observability"`) + the literal `"domain"` |
| dependency not complete | the blocking `"FT-"` id + the failing status (e.g. `"planned"`) |
| spec incomplete | the missing H2 heading (e.g. `"### Behaviour"`) |
| no TCs linked | the literal `"no TCs linked"` |
| TC missing body | the offending `"TC-"` id + the literal `"body"` |
| TC missing runner | the offending `"TC-"` id + the literal `"runner"` |
| VG pending_review | the offending `"VG-"` id |

### Setup

For each gap row, build a stub inspector that returns the relevant ids in
its respective accessor (`unack_xcutting_adrs`, `gap_domains`,
`blocking_deps`, `incomplete_tc_ids`, `pending_review_vg_ids`). Call
`classify("FT-T255", "BNCH-002")` and assert `reason.contains(...)` for
every token in the row.

### Driver passthrough

A `MutableStubInspector` configured for one round of `Stuck { reason: "X" }`
driven via `drive::run` returns `Err::Stuck { reason: "X", history }`. The
reason is byte-identical to the planner's output (no rewrapping, no
prefix-stripping). The CLI adapter then prefixes the final printed line with
`FT-T255 def-ready: X`.

### Boundary

- When multiple ids are responsible (e.g. three pending_review VGs), the
  reason contains all three, comma-separated and sorted ascending for
  determinism. Tests assert sort order so two runs of the same inputs emit
  byte-identical strings.

## Notes

The test enforces the "operators can act without re-running" invariant from
FT-119's error-handling section. Without it, a regression could silently
demote a useful reason like `"TC quality: TC-203 missing runner"` to a
generic `"TC quality issue"` and the operator would have no recourse.
