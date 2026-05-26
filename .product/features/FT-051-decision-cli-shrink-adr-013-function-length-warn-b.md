---
id: FT-051
title: 'decision-cli: shrink ADR-013 function-length warn-band offenders'
phase: 2
status: complete
depends-on: []
adrs: []
tests:
- TC-042
- TC-043
domains: []
domains-acknowledged:
  ADR-024: ADR-024 (feedback lifecycle state machine) is implemented by FT-027; FT-051 produces no feedback artifacts.
  ADR-027: ADR-027 (authority declarations in role catalog) is implemented by FT-030; FT-051 does not introduce or modify a role catalog entry.
  ADR-018: ADR-018 (VerificationVerdict schema) is implemented by FT-020; FT-051 neither emits nor consumes verdicts.
  ADR-022: ADR-022 (feedback as a first-class flow class) is implemented by FT-026; FT-051 is hygiene work that emits no feedback.
  ADR-017: ADR-017 (action-interpretation pairing) is implemented by FT-021; FT-051 is a pure refactor and creates no action/interpretation pair.
  ADR-012: ADR-012 (per-stream working directory discovery) governs CLI entry; FT-051 has no CLI surface and does not alter working-directory resolution.
  ADR-021: ADR-021 (action-interpretation agreement metric) is a Slice 2 fitness function implemented by FT-024; FT-051 is a pure refactor and produces no action/interpretation pair.
  ADR-014: ADR-014 (fitness functions tracked as artifacts) is owned by FT-014/FT-015; FT-051 acts on the diagnostics produced by the function-length fitness check (TC-042/TC-043) but does not author or modify the fitness-function artifact itself.
  ADR-001: ADR-001 governs the oxi-events crate boundary; FT-051 explicitly out-of-scopes `crates/oxi-events/` warn-band offenders and only refactors functions inside `crates/decision-cli/src/` and `workers/`, never crossing the SDP boundary.
  ADR-004: ADR-004 (PROV-O) governs session/event lineage; FT-051 opens no session and emits no events.
  ADR-005: ADR-005 (value-stream scope) governs command-time scope; FT-051 is a pure refactor with no CLI or scope changes.
  ADR-016: ADR-016 (vertical-slice + compile-time SDP) is migrated by FT-018; FT-051 honors that boundary by keeping extractions inside the same module/slice that owns the original function and adds no cross-feature imports.
  ADR-023: ADR-023 (feedback class controlled vocabulary) is implemented by FT-028; FT-051 produces no feedback artifacts.
  ADR-002: ADR-002 (graph-as-state) governs persistence semantics; FT-051 changes no persistence paths and only extracts intra-module helpers.
  ADR-025: ADR-025 (blocking vs non-blocking feedback semantics) is implemented by FT-032; FT-051 has no feedback to gate.
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