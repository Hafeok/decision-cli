---
id: TC-325
title: 'FT-135: drive brackets executor calls with exec start + exec ok/fail lines carrying elapsed time'
type: exit-criteria
status: passing
validates:
  features:
  - FT-135
  adrs: []
phase: 1
runner: cargo-test
runner-args: -p decision-cli ft_135_exec_bracket_lines
runner-timeout: 300
observes:
- stderr
last-run: 2026-06-12T12:53:43.885714192+00:00
last-run-duration: 4.1s
---

## Acceptance criteria

Verifies that [FT-135](FT-135)'s `ProgressSink` brackets every worker-dispatch execution with paired `exec start:` and `exec ok` / `exec fail` stderr lines, with the closing line carrying the elapsed wall-time per the format documented in FT-135's spec body.

### Conditions

Bash runner script captures stderr of a single-feature drive invocation that actually dispatches a worker (so an execution bracket is produced) and asserts the start/end pair format.

- Run `dec drive def-ready FT-110 --max-iter 1 2>capture.log` (FT-110 is `planned` and reaches the verify-graph-author dispatch path on iter 0, guaranteeing at least one execution bracket).
- `capture.log` is non-empty.
- At least one line matches `^\[FT-[0-9]+\] iter [0-9]+ .*exec start: \S+`, where the trailing token is the action / role name (e.g. `verify-graph-author`, `implementer`).
- At least one line on the same feature/iter matches either `^\[FT-[0-9]+\] iter [0-9]+ .*exec ok\s+[0-9]+(\.[0-9]+)?s` or `^\[FT-[0-9]+\] iter [0-9]+ .*exec fail` (with the `ok` variant carrying an elapsed-time token like `8.3s`).
- The `start` and `ok|fail` lines share the same feature id and iter number — the brackets are paired.
- All lines are on stderr; terminal `Done`/`Stuck` history continues to print to stdout unchanged (FT-135 invariant).

### Exit codes

- `0` — at least one paired `exec start` + `exec ok|fail` bracket on stderr.
- `1` — only one half of the bracket present, or formatting drift. Script prints captured stderr for diagnosis.

### Surface

`stderr` — bash runner observes `2>capture.log` redirect from a live `dec` invocation.

### Status note

FT-135 is `planned` at the time of writing. This TC is forward-looking — pair with TC-324 (per-round plan line). Both lock the FT-135 contract surface in advance of implementation so `product verify FT-135` has its runners wired the moment the code lands.