---
id: TC-205
title: Filter flag intersects against resolver list and errors on unknown IDs
type: scenario
status: passing
validates:
  features:
  - FT-111
  adrs: []
observes:
- stdout
phase: 4
runner: cargo-test
runner-args: tc_205_filter_intersects_and_validates
runner-timeout: 30
last-run: 2026-05-29T09:31:56.032729503+00:00
last-run-duration: 0.5s
---

## Description

`--filter FT-A,FT-B,...` narrows the sweep to a subset. Two
properties matter: (a) the intersection preserves the resolver's
numeric-suffix ordering (proves it's an intersect, not a re-order
based on the filter list itself), and (b) an unknown ID in the
filter aborts before any drive runs, with a message naming the
unknown ID.

## Acceptance Criteria

Seed resolver with `[FT-2, FT-3, FT-10, FT-100]` (post-sort).

**Case 1 — intersection preserves order:**
Call `resolve_with_filter(resolver, Some(["FT-10", "FT-2"]))`.
Assert returns `Ok(["FT-2", "FT-10"])` — note FT-2 first
despite appearing later in the filter list.

**Case 2 — unknown ID errors out:**
Call `resolve_with_filter(resolver, Some(["FT-2", "FT-999"]))`.
Assert returns `Err(...)` and the error's display contains
`"FT-999"`.

**Case 3 — empty filter not allowed:**
Call `resolve_with_filter(resolver, Some([]))`. Assert error
(an empty filter is operator confusion, not intent — they meant
`None`).