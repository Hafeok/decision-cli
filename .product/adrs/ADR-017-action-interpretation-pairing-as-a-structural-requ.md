---
id: ADR-017
title: Action-interpretation pairing as a structural requirement
status: proposed
features: []
supersedes: []
superseded-by: []
domains: []
scope: cross-cutting
source-files:
- scripts/checks/action-interpretation-pairing.sh
- scripts/checks/dispatch-complete-paired-terminal.sh
- scripts/checks/dispatch-rejected-stays-blocked.sh
---

## Context

Slice 1 ([ADR-010](ADR-010), [FT-011](FT-011)) shipped the implementer role end-to-end: a `Session` of type `ActionSession` consumes a curated bundle, the worker produces a `CodeChange`, the orchestrator records both with PROV-O lineage, and `dec implement FT-XXX` exits 0 on success.

That is half a DDD cycle. The other half — interpretation — is the act of looking at what the action produced and deciding whether it is in fact what was asked for. Without that step, an action session is its own judge: the implementer reports success if the worker exits 0, and the framework has no structural way to distinguish "code that compiles" from "code that satisfies the feature_spec."

`Implementing_DDD.md` §6 frames this directly: every action is paired with an interpretation that verifies its output. Phase A is the smallest scope that proves the claim end-to-end — so the pairing must be structural (the orchestrator refuses to mark a dispatch complete without it), not advisory (a reviewer might look at it later).

## Decision

**Every action session in decision-cli is structurally paired with an interpretation session. The dispatch is not complete until both sessions terminate.**

The shape:

1. A new graph artifact, `DispatchGroup`, is the parent of both sessions for a single dispatched unit of work. `dec:DispatchGroup` carries the original goal/feature reference, the dispatch identifier, and PROV-O links to the action session and (once minted) the interpretation session.
2. When the action session's worker terminates, the orchestrator transitions the action session to `completed` (success) or `failed` (worker error). On `completed`, the action's output (e.g. `CodeChange`) is **provisional** — it exists in the graph but the dispatch status is `awaiting-interpretation`, not `complete`.
3. The orchestrator dispatches the verifier role (see [ADR-019](ADR-019), [ADR-020](ADR-020)). The verifier's bundle is constructed from: the produced artifact, the originating feature_spec, the bundle hash that produced the action, the relevant TCs, and any cross-cutting ADRs that bound the action.
4. The interpretation session produces a `VerificationVerdict` ([ADR-018](ADR-018)). When the interpretation session terminates with a verdict, the orchestrator transitions the dispatch:
   - `approved` → dispatch `complete`.
   - `amendment-required` → dispatch `awaiting-amendment`; a follow-up action dispatch is required to address the verifier's specific guidance.
   - `rejected` → dispatch `interpretation-rejected`; the produced artifact remains in the graph but is marked superseded for the purposes of feature completion.
5. SHACL refuses to mint a `DispatchGroup` in `complete` status without a matching `InterpretationSession` whose `VerificationVerdict` exists and has status `approved`. The pairing is enforced at write time, not at command time.

### Failure modes the rule handles

- The verifier worker crashes. The interpretation session reaches `failed`; the dispatch reaches `interpretation-failed`. The action's artifact is provisional indefinitely — the operator must rerun verification, not silently accept the action.
- The verifier issues `rejected` but the action artifact (a `CodeChange`, files-on-disk) has already mutated the workspace. The rejection itself is the audit record. Phase A does not auto-revert workspace changes; that policy belongs to a later slice (likely Phase B, once policy is a graph artifact).
- The action session reports `failed` directly. No interpretation is dispatched — there is nothing to interpret. The dispatch transitions to `action-failed` and the `DispatchGroup` carries that terminal status.

### Why "structural" and not "policy"

A policy-driven version would say "dispatches *should* pair action with interpretation, configurable per role." That is the wrong direction: it makes the central claim of the framework optional. By baking the pairing into the dispatch lifecycle, the framework's behavior matches its own claim — you cannot run decision-cli in "no interpretation" mode without ripping out the orchestrator's terminal-state logic.

## Rejected alternatives

- **Inline interpretation (collapsed session).** The action session emits both the artifact and a self-verdict in one shot. Rejected for Phase A: cuts the audit trail in half, and the failure mode "action and interpretation agree because they are the same model in the same context" is exactly what we need to measure (see [ADR-021](ADR-021)). Revisit once the verifier role's behavior is well-characterized ([ADR-019](ADR-019)).
- **Optional interpretation, gated by feature flag.** Rejected: makes the framework's central claim optional. See above.
- **Interpretation as a post-hoc audit job.** Rejected: an artifact that is "complete-pending-verification" creates ambiguity at every other read site (gap_check, drift_check, fitness functions). A dispatch is complete or it isn't.

## Consequences

**Positive:**
- The framework's central claim is observable and falsifiable from day one.
- Every artifact in the graph has a verdict reachable via PROV-O — feature-completion queries become deterministic.
- Action-interpretation disagreement becomes a first-class metric ([ADR-021](ADR-021)) from session 1.

**Negative / accepted costs:**
- Every dispatch costs two LLM calls instead of one. For Phase A this is the correct trade.
- The orchestrator's dispatch lifecycle gains states (`awaiting-interpretation`, `interpretation-rejected`, `awaiting-amendment`). The state diagram is no longer trivial.
- Verifier failures (worker crashes, model unavailability) block dispatch completion. Operator burden is non-zero.

**Enforcement:**
- SHACL shape on `DispatchGroup` (Phase A invariant TC).
- Orchestrator unit + integration tests gating dispatch terminal transitions.
- A cross-cutting TC under [ADR-014](ADR-014) that queries the orchestration store for any `DispatchGroup` in `complete` status without a paired approved verdict — exit 1 if any row.

## Status

Proposed. Foundational for Phase A; every slice-2 feature_spec links to this ADR.
