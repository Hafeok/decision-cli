---
id: TC-158
title: dec verify graph run prints per-step trace, final verdict, and maps verdict to exit code
type: scenario
status: unimplemented
validates:
  features:
  - FT-099
  adrs: []
phase: 1
---

## Claim

Running `dec verify graph run <VG>` against an initialised `.dec/` store:

1. Streams a per-step trace line to stdout for each step in graph order, including kind, outcome, duration, and a short description.
2. Prints a `Verdict: <verdict>` line, the `Result: <path>` to the persisted VGR, and (if any) emitted `Feedback: <FB-NNN>` lines.
3. Returns exit code 0 on `approved`, 1 on `rejected`, 2 on `amendment-required`.

## Scenarios

### Setup

- Fresh `.dec/` initialised via `dec init` against a minimal value-stream fixture.
- Seed env `ENV-FIXTURE-001` (`ephemeral-tempdir`).
- Seed three graphs: `VG-PASS` (one passing shell-command), `VG-FAIL` (one failing shell-command with `providesEvidenceFor = [TC-EVI]`), `VG-AMEND` (one shell-command with `dec:expectExitCode = 0` but the command times out → `unrunnable`).

### Scenario A — approved

Invoke `dec verify graph run VG-PASS`. Assertions:

- Exit code: 0.
- Stdout contains a line matching `^\[0\] shell-command\s+pass\s+\d+ ms` (the per-step row).
- Stdout contains `Verdict: approved` on its own line.
- Stdout contains `Result: .dec/verify/result/VGR-\d+\.ttl` and the referenced file exists.
- Stdout does **not** contain a `Feedback:` line.

### Scenario B — rejected

Invoke `dec verify graph run VG-FAIL`. Assertions:

- Exit code: 1.
- Stdout contains `[0] shell-command fail` and `Verdict: rejected`.
- Stdout contains a `Feedback: FB-\d+` line listing the emitted feedback IRI(s) and `→ TC-EVI`.
- The result file exists and contains `dec:verdict "rejected"`.

### Scenario C — amendment-required

Invoke `dec verify graph run VG-AMEND`. Assertions:

- Exit code: 2.
- Stdout contains `unrunnable` for the step and `Verdict: amendment-required`.

### Scenario D — handler error

Invoke `dec verify graph run VG-DOES-NOT-EXIST`. Assertions:

- Exit code: 1.
- Stderr contains `ArtifactNotFound` or an equivalent "graph not found" message naming `VG-DOES-NOT-EXIST`.
- No `.dec/verify/result/VGR-*.ttl` is created.

### Scenario E — --format json

Invoke `dec verify graph run VG-PASS --format json`. Assertions:

- Exit code: 0.
- Stdout is a single JSON document with keys `session_id`, `result_id`, `verdict`, `step_outcomes`, `emitted_feedback`. `verdict == "approved"`. `step_outcomes` is an array of length 1.

## Runner

`bash tests/scripts/tc-158-dec-verify-graph-run.sh`. The script must:

1. Create a temp `.dec/` via `dec init` in a sandboxed working directory (script `cd`s into a `mktemp -d`).
2. Seed the fixtures via `dec verify env new` / `dec verify graph new` / `dec verify step add` (the slice-2.5 verbs).
3. Invoke each scenario and assert exit code + stdout/stderr pattern matches.
4. Clean up the temp directory on exit.

Exit 0 if all five scenarios pass, exit 1 with a descriptive message on the first failure.

## Non-goals

- MCP transport behaviour (a sibling MCP TC could be added if the MCP surface grows divergent behaviour; for this slice the MCP path is asserted by reusing the same handler under integration tests, not a separate TC).
- Streaming via `--format sse` over a tty (write-only output stream — out of scope for the slice).
- Coverage-gap exit code 3 (TC-160 covers that).
