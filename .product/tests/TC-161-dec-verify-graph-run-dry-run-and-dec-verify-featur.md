---
id: TC-161
title: dec verify graph run --dry-run and dec verify feature --dry-run write no artifacts and open no sessions
type: scenario
status: passing
validates:
  features:
  - FT-099
  adrs: []
phase: 1
runner: bash
runner-args: tests/scripts/tc-161-dec-verify-dry-run.sh
runner-timeout: 240
last-run: 2026-05-26T14:11:38.572067190+00:00
last-run-duration: 0.5s
---

## Claim

`dec verify graph run <VG> --dry-run` and `dec verify feature <FT> --dry-run` enumerate what would be executed, print the enumeration to stdout, and exit 0 **without** writing any `VerificationGraphResult` artifact, opening any `Session` artifact, or invoking the runner's Phase 2/3/4 (no env setup, no step execution, no env teardown).

## Scenarios

### Setup

- Fresh `.dec/` initialised via `dec init`.
- Seed env `ENV-001`, feature `FT-DRY` with TCs `[TC-DRY-A, TC-DRY-B]`, graphs `VG-DRY-1` (covers TC-DRY-A) and `VG-DRY-2` (covers TC-DRY-B). Both graphs contain a `shell-command` step that would create observable side effects (e.g. `touch sentinel-graph-1.txt`).
- Snapshot pre-state: list `.dec/verify/result/*.ttl` (expected empty) and `dec session list` (expected empty of `verify-graph-runner` sessions).

### Scenario A — `dec verify feature FT-DRY --dry-run`

Invoke. Assertions:

- Exit code: 0.
- Stdout enumerates `Would run: VG-DRY-1 (ENV-001)` and `Would run: VG-DRY-2 (ENV-001)`.
- Stdout enumerates `Would reuse: (none)` (no prior VGRs).
- Stdout does **not** contain any per-step trace rows (the runner was never invoked).
- Post-state matches pre-state exactly: `.dec/verify/result/` is still empty, `dec session list` shows no new `verify-graph-runner` sessions.
- The sentinel files (`sentinel-graph-1.txt`, `sentinel-graph-2.txt`) do not exist on disk (proof that no shell-command step ran).

### Scenario B — `dec verify graph run VG-DRY-1 --dry-run`

The graph-level verb does not have a documented `--dry-run` in [FT-099](FT-099); the test must assert one of two behaviours and pin the choice:

- **If the slice ships `--dry-run` on the graph verb too** (recommended for consistency): exit 0, stdout `Would run: VG-DRY-1 (ENV-001)` plus the graph's resolved step list, no artifacts written, no sentinel files created.
- **If the slice ships `--dry-run` only on `feature`**: invoking `dec verify graph run VG-DRY-1 --dry-run` exits non-zero (usage error from `clap`) and the help text does not list `--dry-run` as an option.

The test asserts whichever is consistent with FT-099's final surface; the implementing slice picks one and the test pins it.

### Scenario C — `dec verify feature FT-DRY --dry-run --format json`

Stdout parses as JSON with keys `would_run` (array of `{vg, env}`), `would_reuse` (array, empty here), `dry_run: true`. No artifacts written.

### Scenario D — reuse enumeration after a prior run

1. Run `dec verify feature FT-DRY` (real, not dry-run) to produce two `VerificationGraphResult` artifacts.
2. Within the freshness window (default 24 h), invoke `dec verify feature FT-DRY --dry-run`. Stdout must show `Would reuse: VGR-N (for VG-DRY-1), VGR-M (for VG-DRY-2)` and `Would run: (none)`.
3. Invoke `dec verify feature FT-DRY --dry-run --include-stale`. Stdout must show both as `Would run` again.

## Runner

`bash tests/scripts/tc-161-dec-verify-dry-run.sh`. Same temp-`.dec/` pattern. The script must:

1. Run Scenarios A–C in order against a clean store.
2. Run Scenario D as a separate sequence (real run, then dry-run).
3. Assert exit codes, stdout content, post-state directory listings, and sentinel-file absence/presence.

## Non-goals

- Asserting that the freshness window is exactly 24 h (the value is configurable; the TC only asserts the *behaviour* of reuse-vs-rerun, not the threshold value).
- MCP twin behaviour (the MCP `dry_run: true` form should mirror the CLI; a separate MCP-specific TC is not added in this slice but the same handler is exercised through both routes).
- Coverage-gap interaction with dry-run (dry-run still enumerates; a follow-up TC could assert that `--dry-run` plus a coverage gap exits 3, but the v1 contract is "dry-run always exits 0 once enumeration succeeds").