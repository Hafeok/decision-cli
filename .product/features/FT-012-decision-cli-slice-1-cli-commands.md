---
id: FT-012
title: 'decision-cli: Slice 1 CLI commands'
phase: 1
status: complete
depends-on:
- FT-004
- FT-005
- FT-008
- FT-009
- FT-010
- FT-011
adrs:
- ADR-006
- ADR-010
- ADR-011
- ADR-008
tests:
- TC-006
- TC-007
- TC-008
domains: []
domains-acknowledged:
  ADR-016: ADR-016 (vertical-slice + compile-time SDP) is migrated by FT-018; FT-012's code is reorganised under that migration, not by this feature.
  ADR-013: ADR-013 (code structure standards) applies workspace-wide; FT-012's code conforms to cargo/clippy/rustfmt and the module-size convention. ADR-013 itself is owned by FT-014.
  ADR-018: ADR-018 (VerificationVerdict schema) is a Slice 2 artifact implemented by FT-020; FT-012 neither emits nor consumes verdicts.
  ADR-027: ADR-027 (authority declarations in role catalog) is implemented by FT-030; FT-012 does not introduce or modify a role catalog entry.
  ADR-022: ADR-022 (feedback as a first-class flow class) is a Slice 3 concern implemented by FT-026; FT-012 neither emits nor routes feedback.
  ADR-021: ADR-021 (action-interpretation agreement metric) is a Slice 2 fitness function implemented by FT-024; FT-012 produces no action/interpretation pair.
  ADR-024: ADR-024 (feedback lifecycle state machine) is implemented by FT-027; FT-012 produces no feedback artifacts.
  ADR-014: ADR-014 (fitness functions tracked as artifacts) is owned by FT-014/FT-015; FT-012 does not author or modify a fitness-function artifact.
  ADR-023: ADR-023 (feedback class controlled vocabulary) is implemented by FT-028; FT-012 produces no feedback artifacts.
  ADR-017: ADR-017 (action-interpretation pairing) is a Slice 2 structural requirement implemented by FT-021; FT-012 is out of scope for the pairing.
  ADR-025: ADR-025 (blocking vs non-blocking feedback semantics) is implemented by FT-032; FT-012 has no feedback to gate.
---

## Description

The slice 1 `dec` CLI exposes a minimal subset of the single-binary namespaced surface defined by **ADR-011**: bootstrap, identity / health, explicit trigger per **ADR-010**, inspection. Bootstrap follows the **ADR-006** validation pipeline (no raw-string overrides).

The full `drive` / `watch` / `schedule` / `dispatch role` vocabulary lands in later slices (ADR-011).

See `decision-cli-slice-1-bounds.md` §6.1, §9.

## Functional Specification

### Inputs

- Command-line arguments parsed via `clap` (or equivalent).
- The active orchestration store discovered per FT-009 (ADR-012).

### Outputs

- Structured stdout per command (human-readable; JSON output deferred).
- Non-zero exit codes on failures.
- Side-effects: init writes to store; implement triggers dispatch; others read-only.

### State

- Stateless — all state comes from the orchestration store.

### Behaviour

- `dec init --template <name>` / `dec init --from <path>` — delegate to FT-008 (ADR-006 validation pipeline).
- `dec status` — active ValueStream, definition source + hash, terminal value action, authorized goals, recent-session count, in-flight dispatch count (per §3.7).
- `dec health` — liveness (store opens, ontology parses, writer operational).
- `dec implement FT-XXX` — delegate to FT-011 (ADR-010 explicit trigger).
- `dec events tail` — connect to FT-004 SSE endpoint and stream live events.
- `dec events since <seq>` — call FT-005 replay and print events.
- `dec session list` — paginated recent sessions from the store.
- `dec session show <id>` — full session details with bundle hash and output ref.
- `dec session log <id>` — PROV-O chain for a session (ADR-004).

### Invariants

- Every command except `dec init` and `dec health` refuses cleanly when run outside a `.dec/` working tree (ADR-012).
- Help text for each command names prerequisites and failure modes.
- Exit codes stable per error category (ADR-011 follows `sysexits`).

### Error handling

- Missing store → exit code 2, hint `dec init`.
- Argument parse error → exit code 64 (sysexits EX_USAGE).
- Runtime failures from FT-008/FT-009/FT-010/FT-011 → exit code 1 with the structured error message from the underlying feature.

### Boundaries

- Does NOT implement business logic for any command — composes the features that do.
- The full goal-oriented vocabulary (`dec drive`, `dec watch`, `dec schedule`, `dec dispatch role`) is out of scope for slice 1 per ADR-010 and ADR-011.

## Out of scope

- `dec drive`, `dec watch`, `dec schedule`, `dec dispatch role`, `dec checkpoint`, `dec stream`, `dec product`.
- JSON output mode.
- Shell completion generation.
