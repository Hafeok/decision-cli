---
id: FT-031
title: Worker emit_feedback SDK (Python + Rust)
phase: 2
status: complete
depends-on:
- FT-013
- FT-026
- FT-027
- FT-028
- FT-030
adrs:
- ADR-008
- ADR-022
- ADR-027
tests:
- TC-041
domains: []
domains-acknowledged:
  ADR-013: ADR-013 (code structure standards) applies workspace-wide; FT-031's code conforms to cargo/clippy/rustfmt and the module-size convention. ADR-013 itself is owned by FT-014.
  ADR-014: ADR-014 (fitness functions tracked as artifacts) is owned by FT-014/FT-015; FT-031 does not author or modify a fitness-function artifact.
  ADR-018: ADR-018 (VerificationVerdict schema) is a Slice 2 artifact implemented by FT-020; FT-031 neither emits nor consumes verdicts.
  ADR-012: ADR-012 (per-stream working directory discovery) governs CLI entry; FT-031 runs after the working directory is resolved and does not re-discover it.
  ADR-002: ADR-002 (graph-as-state) governs persistence semantics; FT-031 reads/writes via the GraphWriter chokepoint and does not introduce event-sourced state.
  ADR-021: ADR-021 (action-interpretation agreement metric) is a Slice 2 fitness function implemented by FT-024; FT-031 produces no action/interpretation pair.
  ADR-001: ADR-001 governs the oxi-events crate boundary; FT-031 does not cross or alter that boundary.
  ADR-017: ADR-017 (action-interpretation pairing) is a Slice 2 structural requirement implemented by FT-021; FT-031 is out of scope for the pairing.
  ADR-004: ADR-004 (PROV-O) governs session/event lineage; FT-031 produces no new Session or event type and inherits lineage from the harness.
  ADR-016: ADR-016 (vertical-slice + compile-time SDP) is migrated by FT-018; FT-031's code is reorganised under that migration, not by this feature.
  ADR-023: ADR-023 (feedback class controlled vocabulary) is implemented by FT-028; FT-031 produces no feedback artifacts.
  ADR-025: ADR-025 (blocking vs non-blocking feedback semantics) is implemented by FT-032; FT-031 has no feedback to gate.
  ADR-024: ADR-024 (feedback lifecycle state machine) is implemented by FT-027; FT-031 produces no feedback artifacts.
  ADR-005: ADR-005 (value-stream scope) governs command-time scope; FT-031 runs inside an already-scoped command and does not introduce a new scope check.
---

## Description

Extend the worker SDK in both Python (`workers/code-writer/`, `workers/verifier/`, future workers) and the Rust harness with a structured `emit_feedback` call. Workers use this to surface emergent decisions ([ADR-022](ADR-022)) without writing to the graph directly ([ADR-008](ADR-008) holds — workers remain stateless and graph-blind).

## Functional Specification

### Inputs

- The feedback schema from [FT-026](FT-026).
- The class vocabulary from [FT-028](FT-028).
- The lifecycle state machine from [FT-027](FT-027) (workers always produce in state `produced`).
- The role authority declaration from [FT-030](FT-030) (workers consume it from the bundle).

### Outputs

- **Python SDK** (`workers/_shared/feedback.py` — a new shared package for worker utilities):
  ```python
  from pydantic import BaseModel
  from typing import Literal

  FeedbackClass = Literal["gap", "contradiction", "unimplementable",
                          "scope-issue", "defect", "capability-request"]

  class FeedbackEmission(BaseModel):
      feedback_class: FeedbackClass
      severity: Literal["low", "medium", "high"]
      evidence: str           # ≥ 20 chars
      recommendation: str | None = None
      target_role_override: str | None = None
      blocking: bool | None = None     # None = use class default
      disposition_rationale: str | None = None  # required if blocking != default

  def emit_feedback(emission: FeedbackEmission) -> None:
      """Serialise FeedbackEmission to a structured stdout record the harness reads."""
  ```
- Each worker (`code-writer`, `verifier`) imports `emit_feedback` from the shared package. The worker calls it inline during action; the SDK serialises a JSON record to stdout (one per emission) on a separate stream the harness parses.
- **Rust harness side** (`core::worker::ipc::feedback`):
  - Parses emitted JSON records from worker stdout.
  - For each record, constructs a `core::feedback::artifact::Feedback` in state `produced`, populates `sourceSession` from the active session, validates the class/category against the role's authority declaration, writes through `StreamWriter`.
  - If `blocking = true` (explicit or class-default), signals the dispatch lifecycle to enter `paused-for-feedback` ([FT-032](FT-032)).
- **Bundle injection** of authority:
  - The harness, when assembling a worker bundle, includes the role's authority declaration ([FT-030](FT-030)) as a structured section.
  - Worker SDK exposes `Bundle.authority` (Pydantic field) so worker prompts can render the may-decide / must-escalate lists.

### State

- Per-emission: one new `Feedback` artifact in state `produced` through `StreamWriter`.
- Session telemetry: each emission is recorded in the session's tool-call log so Phase C can compute feedback-per-session metrics.

### Behaviour

1. Create the shared package `workers/_shared/` with `feedback.py`, `bundle.py` (shared bundle parsing), `output.py` (shared exit protocol). Both `code-writer` and `verifier` depend on it.
2. Author the Python `emit_feedback` API and the wire format (newline-delimited JSON on a dedicated stdout channel, prefixed with a sentinel like `__DEC_FEEDBACK__`).
3. Author the Rust parser `core::worker::ipc::feedback::parse_records(stream) -> Vec<FeedbackEmission>` and the writer `apply(store, session_iri, emission)`.
4. Integrate parser into the slice-1 worker harness (the path that runs `code-writer`): after the worker exits, scan stdout for feedback records, apply each through `StreamWriter`.
5. Integrate parser into the slice-2 verifier harness (FT-022 → FT-023 path): same pattern.
6. The harness validates each emission against the role's authority before writing:
   - If the emission's class corresponds to a `mustEscalate` category for the role: OK.
   - If the emission's class corresponds to a `mayDecide` category: log a warning (over-cautious worker) and write anyway.
   - If the class doesn't correspond to either: log a warning and write anyway (the worker has invented a category — surface for Phase C analysis).
7. Per the slice-level SDP: the Rust parser lives in `core::worker::ipc::feedback`. Slice-2 and slice-3 features consume it; no sibling feature reaches in.

### Invariants

- Every emission produces exactly one `Feedback` artifact in state `produced`.
- The artifact's `dec:sourceSession` matches the active session at emission time.
- Workers never write feedback through any path other than the SDK (enforced by [ADR-008](ADR-008)'s no-graph-access invariant — workers physically cannot reach the store).
- The shared package contains no project-Rust imports — pure Python (worker contract invariant).

### Error handling

- Malformed emission JSON from worker → harness logs the error and continues; the bad emission is dropped (the worker's session telemetry includes the parse failure for Phase C analysis).
- Worker emits with unknown class → harness logs and writes anyway (defensive: SHACL will catch it on write); the worker is signalled out-of-vocab via the warning in its next bundle (Phase B at earliest).
- `StreamWriter` SHACL rejection of an emission → harness records the rejection in session telemetry; the original worker call has already returned (workers cannot retry).
- Worker emits blocking feedback then continues to produce an action artifact: the harness drops the action artifact and treats the dispatch as `paused-for-feedback` (per [ADR-025](ADR-025) — blocking emissions abort the action structurally).

### Boundaries

- **In scope.** Python SDK, Rust parser, harness integration, authority validation, bundle injection of authority.
- **Out of scope.** The lifecycle state machine ([FT-027](FT-027)). The routing subscription ([FT-029](FT-029)). The pause/resume dispatch lifecycle changes ([FT-032](FT-032)). CLI surfaces ([FT-033](FT-033)).

## Out of scope

- A Rust worker SDK (no Rust workers in Phase A; the SDK shape is identical when added later).
- Async / streaming feedback emission (Phase B at earliest).
- Workers reading their own prior emissions (rejected — workers are stateless).
