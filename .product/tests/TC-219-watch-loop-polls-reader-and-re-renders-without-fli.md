---
id: TC-219
title: Watch loop polls reader and re-renders without flicker, exits cleanly on SIGINT
type: scenario
status: passing
validates:
  features:
  - FT-113
  adrs: []
observes:
- stdout
- exit-code
phase: 4
runner: cargo-test
runner-args: tc_219_watch_loop_polls_and_exits_on_sigint
runner-timeout: 30
last-run: 2026-05-30T15:07:13.666786708+00:00
last-run-duration: 0.4s
---

## Description

The watch loop is the live-dashboard half of FT-113. Three
behaviours matter: it polls the reader on the configured
interval, it clears the screen between frames so the latest
render replaces the previous (no scrollback flicker), and it
exits cleanly when the operator hits Ctrl-C.

## Acceptance Criteria

Cargo test using `tokio::test` with `start_paused = true`:

1. Stub the reader so its first call returns one round and
   subsequent calls return two rounds (simulates a drive
   that emitted a new round between polls).
2. Launch the watch loop in a tokio task with
   `--interval = 1s`.
3. Advance time by 1.5s. Capture the rendered output stream.
   Assert two distinct frames have been emitted (first frame
   contains one round, second contains two).
4. Assert each frame begins with the ANSI clear-screen
   sequence (or the equivalent `\x1b[2J\x1b[H`).
5. Send a simulated SIGINT to the task.
6. Assert the task exits within 100ms with return value
   indicating clean shutdown.
7. Assert the final rendered output contains the substring
   `"stopped"`.

No real wall-clock sleeps; `tokio::time::pause` makes the test
deterministic and fast.