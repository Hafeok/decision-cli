---
id: TC-022
title: oxi_events_sdp_boundary_is_intact
type: invariant
status: passing
validates:
  features: []
  adrs:
  - ADR-001
phase: 1
runner: bash
runner-args: scripts/checks/oxi-events-sdp-boundary.sh
runner-timeout: 60
last-run: 2026-05-20T08:26:41.315265110+00:00
last-run-duration: 0.0s
---

## Purpose

Mechanical enforcement of **ADR-001 Stable Dependency Principle**. Asserts
that the `oxi-events` crate does not depend on `decision-cli` and does not
reference DDD vocabulary (`role_id`, `RoleBinding`, `bundle_hash`,
`session_id`, `policy_id`, `autonomy_level`) in its sources.

`oxi-events` is the substrate; DDD concepts live downstream in
`decision-cli`. The SDP boundary keeps the substrate reusable.

## Given

- A working copy of decision-cli with both `crates/oxi-events/` and
  `crates/decision-cli/` checked out.
- `bash`, `grep`, and `git` available on `PATH`.

## When

```bash
scripts/checks/oxi-events-sdp-boundary.sh
```

## Then

1. Exit 0 if `crates/oxi-events/Cargo.toml` does not declare a
   `decision-cli` dependency and no source file under
   `crates/oxi-events/src/` references the forbidden DDD vocabulary
   outside of comment lines.
2. Exit 1 otherwise; diagnostic lines on stdout name the offending
   file:line and term.

## Notes

- The check intentionally allows the forbidden terms in comments so the
  module headers can *explain why* those terms are forbidden.
- Authoring a new DDD-aware feature inside `oxi-events` will surface as
  an exit-1 here long before the SDP boundary erodes.