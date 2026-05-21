---
id: FT-051
title: 'decision-cli: shrink ADR-013 function-length warn-band offenders'
phase: 2
status: planned
depends-on: []
adrs:
- ADR-013
tests:
- TC-042
- TC-043
domains: []
domains-acknowledged: {}
---

## Description

Bring every first-party function body in `crates/decision-cli/src/` and
`workers/*/` below the **30 statement-line** ADR-013 warn threshold so a
clean tree produces **zero** `WARNING:` diagnostics from
`scripts/checks/function-length.sh` and `scripts/checks/function-length.py`.

The hard 40-line gate is already enforced (no offender exceeds it today).
This feature is the soft-tier hygiene work that was deferred when ADR-013
was amended to a two-tier exit-code contract (see the amendment audit on
ADR-013 and the "Earlier revisions…" notes in TC-016, TC-042, TC-043).

## Why this exists separately

ADR-013's amended contract reports warn-band offenders as advisory stdout
diagnostics rather than as a separate exit code. That keeps the fitness
TCs runnable, but it also turns the warn band into a "passively visible"
signal — easy to ignore. This feature_spec converts the diagnostics into
a tracked piece of work so the warn band does not silently grow.

## Current offenders (snapshot, regenerate before scheduling work)

`scripts/checks/function-length.sh` and `function-length.py` are the
source of truth. As of 2026-05-21 the warn-band cohort is:

- **Rust** (`crates/decision-cli/src/`): ~27 functions in the 31–40 line
  band. Hotspots include `core/subscriptions/verifier_dispatch/mod.rs`
  (`seed_quads`, `build_event_payload_quads`),
  `core/feedback/{read.rs, routing/handler.rs, transition.rs, artifact.rs}`,
  `core/dispatch/pause.rs`, `core/metrics/agreement.rs`,
  `features/feedback/{route.rs, list.rs, close.rs}`,
  `features/implement/session_show/paired.rs`,
  `core/stream_writer.rs`, `core/ontology/{verdict/shacl.rs, verdict/read.rs, helpers.rs}`,
  `core/worker/ipc/feedback.rs`, `core/role_catalog/authority.rs`,
  `core/dispatch/group.rs`.
- **Python** (`workers/`): 1 function — `_extract_json_object` in
  `workers/verifier/src/verifier/worker.py:173` (36 statement nodes).

Run the scripts before starting work; the cohort changes as code lands.

## Scope

In scope:

- Apply ADR-013 Rule 2's prescribed remedy ("name the sub-operation and
  extract it") to every warn-band function in `crates/decision-cli/src/`
  and `workers/`.
- Add or move helper functions into the same module where they belong
  (no cross-feature imports per ADR-016 vertical-slice SDP).
- Update or split adjacent unit-test modules if extraction changes
  function boundaries.

Out of scope:

- `crates/oxi-events/` warn-band offenders, if any — handled separately
  to honor the SDP boundary at the crate level.
- Changing the thresholds. `FN_LENGTH_HARD=40` and `FN_LENGTH_WARN=30`
  are inherited from ADR-013 and are not relitigated here.
- Refactoring file structure beyond what extraction requires. File-length
  warn-band (Rule 1) is a different cleanup; if it overlaps,
  cross-reference but do not bundle.

## Acceptance

`scripts/checks/function-length.sh` and `python3 scripts/checks/function-length.py`
both run on a clean tree and produce **no** `WARNING:` lines. Exit code
stays 0. The change set introduces no new hard violations in either
language.

## Notes

- This is hygiene; it does not gate phase 1 closure. The TC pair
  (TC-042, TC-043) already exits 0 with warn-band diagnostics, which is
  what the amended ADR-013 contract permits.
- Sequence: prefer many small commits (one offender or one cohesive
  extraction per commit) over a single sweeping refactor. ADR-013 Rule 2
  argues that each extraction is independently valuable — preserve that
  in the history.