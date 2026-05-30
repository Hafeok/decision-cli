---
id: TC-216
title: Empty feature history renders the no-rounds empty-state paragraph
type: scenario
status: passing
validates:
  features:
  - FT-113
  adrs: []
observes:
- stdout
phase: 4
runner: cargo-test
runner-args: tc_216_empty_history_renders_empty_state
runner-timeout: 30
last-run: 2026-05-30T15:07:13.666786708+00:00
last-run-duration: 0.5s
---

## Description

A feature that's never been driven must render something other
than an empty buffer — an operator with a blank screen can't
distinguish "no rounds" from "render bug." The empty-state
paragraph names the missing data and tells the operator what
to do next.

## Acceptance Criteria

Cargo test:

1. Call `render_text(&[], &RenderOpts::default_for("FT-X", None))`.
2. Assert returned `String` is non-empty.
3. Assert it contains the substring `"No drive history"` and
   the feature id `"FT-X"`.
4. Assert it contains the substring `"dec drive ship FT-X"` —
   the actionable next-step suggestion.
5. Assert no `"Round 0"` substring appears.

Pure-function test; no async runtime, no store.