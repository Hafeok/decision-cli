---
id: TC-217
title: Renderer is pure function of rounds plus options, deterministic across runs
type: invariant
status: passing
validates:
  features:
  - FT-113
  adrs: []
observes:
- stdout
phase: 4
runner: cargo-test
runner-args: tc_217_renderer_is_pure_and_deterministic
runner-timeout: 30
last-run: 2026-05-30T15:40:39.135545566+00:00
last-run-duration: 0.5s
---

## Description

The renderer is the operator-facing contract; reviewers and
downstream tooling rely on byte-stable output for fixture
diffs, log captures, and chat pastes. Implementations are
free to refactor internals, but the same (rounds, options)
must produce the same string.

## Acceptance Criteria

Cargo test:

1. Hand-construct a `Vec<Round>` with one of each interesting
   round shape: VGA produces VG-id, implementer produces
   commit + addressed-count, verifier produces per-TC
   summary, stuck/done terminal states.
2. Call `render_text(&rounds, &opts)` twice in succession.
3. Assert both outputs are byte-identical.
4. Snapshot the output against `tests/fixtures/drive-show-text.txt`
   so PR diffs surface any unintended renderer drift.
5. Reorder the hand-constructed `Vec<Round>` (swap two
   adjacent rounds). Assert the output differs (proves the
   renderer responds to input rather than producing a
   constant).

The renderer takes `now: DateTime<Utc>` as part of `RenderOpts`
so the "elapsed" annotations are deterministic.