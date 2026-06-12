---
id: TC-324
title: 'FT-135: drive emits per-round plan line on stderr with feature id and action tag'
type: exit-criteria
status: passing
validates:
  features:
  - FT-135
  adrs: []
phase: 1
runner: cargo-test
runner-args: -p decision-cli ft_135_plan_line_per_round
runner-timeout: 300
observes:
- stderr
last-run: 2026-06-12T12:53:43.885714192+00:00
last-run-duration: 152.7s
---

## Acceptance criteria

Verifies that [FT-135](FT-135)'s `ProgressSink` emits a per-round line to stderr identifying the feature being driven and the action the planner picked, in the form documented in FT-135's spec body.

### Conditions

Bash runner script captures stderr of a single-feature drive invocation against a fixture or live `.dec/` and asserts at least one line matches the per-round plan format.

- Run `dec drive ship FT-001 --max-iter 1 2>capture.log` (FT-001 is shipped, so the planner exits at iter 0 with `plan=Done` — the goal here is to assert a line is emitted, not to drive an unshipped feature).
- `capture.log` is non-empty.
- At least one line matches the regex `^\[FT-[0-9]+\] iter [0-9]+ .*plan=\S+`, where:
  - The bracketed prefix carries the feature id (so a `--all` sweep's interleaved output stays attributable per feature).
  - `iter N` is the planner round number.
  - `plan=<ActionTag>` carries the planner's classified action name (e.g. `Done`, `DispatchVerifyGraphAuthor`, `DispatchImplementer`).
- The line is on stderr, not stdout — `dec drive ship`'s terminal `Done`/`Stuck` history continues to print to stdout unchanged (FT-135 invariant).

### Exit codes

- `0` — at least one matching per-round plan line on stderr.
- `1` — no matching line. Script prints the captured stderr for diagnosis.

### Surface

`stderr` — bash runner observes `2>capture.log` redirect from a live `dec` invocation.

### Status note

FT-135 is `planned` at the time of writing — this TC is forward-looking. It exists so `dec drive def-ready FT-135` can pass once FT-135 ships, and so `product verify FT-135` has the runner in place at implementation time.