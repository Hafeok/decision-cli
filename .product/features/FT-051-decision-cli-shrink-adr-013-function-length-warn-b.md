---
id: FT-051
title: 'decision-cli: shrink ADR-013 function-length warn-band offenders'
phase: 2
status: planned
depends-on: []
adrs: []
tests:
- TC-042
- TC-043
domains: []
domains-acknowledged:
  ADR-025: ADR-025 (blocking vs non-blocking feedback semantics) is implemented by FT-032; FT-051 has no feedback to gate.
  ADR-017: ADR-017 (action-interpretation pairing) is implemented by FT-021; FT-051 is a pure refactor and creates no action/interpretation pair.
  ADR-002: ADR-002 (graph-as-state) governs persistence semantics; FT-051 changes no persistence paths and only extracts intra-module helpers.
  ADR-005: ADR-005 (value-stream scope) governs command-time scope; FT-051 is a pure refactor with no CLI or scope changes.
  ADR-023: ADR-023 (feedback class controlled vocabulary) is implemented by FT-028; FT-051 produces no feedback artifacts.
  ADR-012: ADR-012 (per-stream working directory discovery) governs CLI entry; FT-051 has no CLI surface and does not alter working-directory resolution.
  ADR-001: ADR-001 governs the oxi-events crate boundary; FT-051 explicitly out-of-scopes `crates/oxi-events/` warn-band offenders and only refactors functions inside `crates/decision-cli/src/` and `workers/`, never crossing the SDP boundary.
  ADR-018: ADR-018 (VerificationVerdict schema) is implemented by FT-020; FT-051 neither emits nor consumes verdicts.
  ADR-014: ADR-014 (fitness functions tracked as artifacts) is owned by FT-014/FT-015; FT-051 acts on the diagnostics produced by the function-length fitness check (TC-042/TC-043) but does not author or modify the fitness-function artifact itself.
  ADR-024: ADR-024 (feedback lifecycle state machine) is implemented by FT-027; FT-051 produces no feedback artifacts.
  ADR-016: ADR-016 (vertical-slice + compile-time SDP) is migrated by FT-018; FT-051 honors that boundary by keeping extractions inside the same module/slice that owns the original function and adds no cross-feature imports.
  ADR-022: ADR-022 (feedback as a first-class flow class) is implemented by FT-026; FT-051 is hygiene work that emits no feedback.
  ADR-004: ADR-004 (PROV-O) governs session/event lineage; FT-051 opens no session and emits no events.
  ADR-027: ADR-027 (authority declarations in role catalog) is implemented by FT-030; FT-051 does not introduce or modify a role catalog entry.
  ADR-021: ADR-021 (action-interpretation agreement metric) is a Slice 2 fitness function implemented by FT-024; FT-051 is a pure refactor and produces no action/interpretation pair.
---

## Description

Bring every first-party function body in `crates/decision-cli/src/` below the **40 statement-line** ADR-013 hard limit, and as a stretch goal below the **30 statement-line** warn threshold. A clean tree produces **zero** `ERROR:` diagnostics and ideally **zero** `WARNING:` diagnostics from `scripts/checks/function-length.sh`.

This feature was previously marked `complete` while four hard-limit violations were still present in the tree — `parse_worker_image`, `validate_subject`, `fetch_query_template`, `list_query_templates`. That's a regression against the feature's own scope. This re-author corrects the status (now `planned`), updates the offender snapshot to current reality, and re-asserts the acceptance criterion as exit 0 on the fitness runner.

## Why this exists separately

ADR-013's amended contract reports warn-band offenders as advisory stdout diagnostics rather than as a separate exit code. That keeps the fitness TCs runnable, but it also turns the warn band into a "passively visible" signal — easy to ignore. The hard band (>40 lines) is gated; ignoring it blocks Phase 3.

## Current offenders (2026-05-26, regenerate before scheduling work)

`scripts/checks/function-length.sh` is the source of truth. Today's hard-limit (>40 statement lines) cohort:

| Function | File | Statement lines | Over |
|---|---|---|---|
| `parse_worker_image` | `crates/decision-cli/src/core/ontology/worker_image/read.rs` | 87 | +47 |
| `fetch_query_template` | `crates/decision-cli/src/core/queries/full_chain.rs` | 52 | +12 |
| `validate_subject` | `crates/decision-cli/src/core/ontology/worker_image/shacl.rs` | 52 | +12 |
| `list_query_templates` | `crates/decision-cli/src/core/queries/full_chain.rs` | 46 | +6 |

The four are concentrated in two files (`worker_image/` and `queries/full_chain.rs`), making file-grouped commits a natural sequencing.

Warn-band (30-40 lines) cohort exists separately and is *not* gating — but the hygiene work in §Acceptance addresses it too as a stretch goal.

## Scope

In scope:

- Apply ADR-013 Rule 2's prescribed remedy ("name the sub-operation and extract it") to **every hard-limit violation** above.
- Add or move helper functions into the same module where they belong (no cross-feature imports per ADR-016 vertical-slice SDP).
- Update or split adjacent unit-test modules if extraction changes function boundaries.
- After the hard-limit work, optionally tackle warn-band offenders that share a file with hard-limit ones (cheap to clean up while context is loaded).

Out of scope:

- `crates/oxi-events/` warn-band offenders, if any — handled separately to honor the SDP boundary at the crate level.
- Changing the thresholds. `FN_LENGTH_HARD=40` and `FN_LENGTH_WARN=30` are inherited from ADR-013 and are not relitigated here.
- Refactoring file structure beyond what extraction requires. File-length warn-band (Rule 1) is a different cleanup; if it overlaps, cross-reference but do not bundle.

## Acceptance

1. `scripts/checks/function-length.sh` exits 0 — no `ERROR:` lines, no functions over 40 statement lines.
2. `scripts/checks/run-all-fitness.sh` exits 0 — TC-086 (code-structure fitness all-green) passes.
3. `cargo test -p decision-cli --lib` passes; no test regressions from the extractions.
4. The change set introduces **no new** hard violations in either language.

Once these hold, mark TC-042 + TC-086 `passing` via `product test status`, then `product feature status FT-051 complete`.

## Notes

- This is hygiene; the surrounding behavior is unchanged. Every extraction is mechanical — group N statements into a helper, give it a name, replace the original block with a call.
- Sequence: prefer many small commits (one file or one cohesive extraction per commit) over a single sweeping refactor. ADR-013 Rule 2 argues that each extraction is independently valuable — preserve that in the history.
- The historical commit that flipped FT-051 to `complete` prematurely is part of the audit trail; this re-author fixes the status without rewriting that commit.
