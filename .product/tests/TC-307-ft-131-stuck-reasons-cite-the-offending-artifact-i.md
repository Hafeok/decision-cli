---
id: TC-307
title: FT-131 Stuck reasons cite the offending artifact id verbatim through the driver
type: scenario
status: passing
validates:
  features:
  - FT-131
  adrs:
  - ADR-076
phase: 1
runner: cargo-test
runner-args: -p decision-cli --test ft_131_stuck_reasons
runner-timeout: 120
observes:
- exit-code
- stdout
last-run: 2026-06-04T09:34:27.854942760+00:00
last-run-duration: 0.2s
---

## Purpose

Validates FT-131 (FeatureReadyPlanner) against ADR-076's debuggability invariant: every Stuck reason emitted by the planner must cite the offending artifact id verbatim so operators can grep the readiness output and jump straight to the artifact. Mirrors TC-255 for FT-119.

## Acceptance

- Stuck reason for a rejected TC contains the substring `"tc rejected: TC-XXX"` (exact TC id).
- Stuck reason for a blocked-by-dep failure contains the substring `"blocked: FT-Y status=<actual-status>"` (exact FT id and status).
- Stuck reason for a VG awaiting human review contains the substring `"vg pending_review: VG-Z"` (exact VG id).
- Each Stuck reason is a single line of structured text suitable for grep, with the artifact id verbatim (no truncation, no aliasing).
- The test asserts on the driver's stdout / Action enum's Display impl, not on the internals of `classify`.

## Inputs

`StubInspector` fixtures per stuck scenario: (a) tcs_rejected=[TC-XXX], (b) blocked_by=[(FT-Y, "complete-blocked")], (c) vgs_pending_review=[VG-Z]. The test invokes `planner.classify(&stub)` and inspects the `Action::Stuck(reason)` payload string.

## Out of scope

- The full classification matrix (covered by TC-306).
- The dispatch-on-non-Stuck paths (covered by TC-308 / TC-309).