---
id: ADR-031
title: Chain-integrity invariant for worker dispatch
status: accepted
features: []
supersedes: []
superseded-by: []
domains: []
scope: domain
content-hash: sha256:4f244b0900c46b469ad131f41aaefaae6a24ba28233267cb13c6419e9b151292
---

## Context

Decision-Driven Design rests on a closed chain of context per piece of work: a feature_spec describes intent → TCs encode acceptance criteria → an implementer produces code → a verifier (slice 3) judges the code against the TCs by **executing a `dec:VerificationGraph`** that asserts each TC's claim. The chain only closes when each link exists. If the implementer is dispatched without a verification graph for the feature, the verifier has nothing to execute — the loop is open, and the system is, in DDD terms, no longer goal-directed: it's *guessing* whether the work satisfied the intent.

[ADR-030](ADR-030) introduces the verify-graph-author role, which makes graph authoring tractable for every feature. But authoring is not enforcement. Even with the author worker available, an unguarded `dec implement` can dispatch the implementer against a feature whose TCs have no covering graph. That's the gap this ADR closes.

This is **not** a slice-2.6-only concern. The implementer is the first worker we have; the same gate must apply to every future worker that acts on a feature (refactor, doc-writer, etc.). The decision is general, not implementer-specific.

## Decision

A dispatch-time invariant — the **chain-integrity invariant** — is asserted on every worker dispatch that targets a `feature` artifact: the harness refuses to dispatch when the target feature's TCs do not have full coverage by at least one `dec:VerificationGraph` per relevant environment, **unless** the caller explicitly waives coverage with a recorded waiver artifact.

Concretely:

1. **Where the gate runs.** Inside `core::harness::dispatch`, immediately before the worker is invoked. The gate consumes the coverage primitive from [FT-045](FT-045) and is implemented as [FT-047](FT-047). It is **not** in the CLI parser; the same gate fires whether the dispatch was triggered manually (`dec implement FT-007`), by subscription, or by a future programmatic API. One chokepoint, one rule.

2. **What "covered" means.** A feature `F` is covered iff for every TC `T` listed in `F.tests`, there exists at least one `dec:VerificationGraph G` such that some `dec:VerificationStep` in `G` declares `dec:providesEvidenceFor T` (the predicate added by [ADR-028](ADR-028)'s amendment under [ADR-030](ADR-030)). The graph's environment is not yet asserted by the gate — environment selection is a slice-3 graph-execution concern. If no graph references a given TC at all, the gate fails closed.

3. **Failure mode.** The gate emits `Error::ChainIntegrity { feature, uncovered_tcs }` and the dispatch returns exit 1 (CLI) or a structured MCP error. The session is not created; no PROV-O activity is opened. The CLI message lists the uncovered TCs and suggests `dec verify graph generate <feature-id> --environment <env>` ([FT-049](FT-049)) as the next step.

4. **Waiver as an artifact, not a flag.** When coverage is *deliberately* impossible — a feature whose TCs are documentary, exploratory, or non-executable, or a one-off urgent change where the team accepts the risk — the dispatch can be forced via `--waive-coverage <reason>`. The flag does **not** silently bypass the gate; it writes a `dec:CoverageWaiver` artifact:
   - `dec:waiverFor F` (the feature)
   - `dec:waiverReason "<reason>"` (mandatory; minimum 16 chars)
   - `prov:wasAttributedTo` the dispatching identity
   - `dcterms:created` UTC timestamp
   - Stored under `.dec/verify/waivers/CW-NNN.ttl`
   The gate then sees the waiver and lets the dispatch through, recording the waiver IRI in the session's PROV-O chain ([ADR-004](ADR-004)). Waivers are listable (`dec verify waivers list` — out of scope for slice 2.6; defer to slice 3) and visible in fitness-function audits.

5. **MCP parity.** The MCP equivalent is `accept_waiver: { reason: string }` in the dispatch tool's input. Same handler, same waiver write path. Per [ADR-029](ADR-029) single-handler discipline.

6. **Fitness-function corollary.** This ADR is the architectural assertion; a slice-3 fitness function will measure waiver rate ("≤ 5 % of feature-dispatches in a rolling 30-day window may invoke `--waive-coverage`"). The metric belongs in [ADR-014](ADR-014)'s registry, not in this ADR — listing it here would couple the architectural rule to a tuning parameter. The metric is mentioned for cross-reference only.

## Consequences

**Positive:**

- The implementer cannot run blind. By construction, every implementer dispatch has a known set of executable acceptance claims (the steps of its covering graphs), so the verifier (slice 3) has something to interpret. The action-interpretation pairing ([ADR-017](ADR-017)) is no longer aspirational at the feature level.
- The gate is one piece of code, one error type, one waiver shape — applied uniformly to every worker. As new worker roles land (refactor, doc-writer, migrator), they inherit the gate without code changes; their dispatch call already goes through `core::harness::dispatch`.
- Waivers as artifacts (not flags) means the team can *measure* how often the gate is bypassed and *justify* each bypass after the fact. A flag would be invisible; an artifact has a content-hash and a PROV-O record.
- Combined with [ADR-030](ADR-030)'s verify-graph-author, the gate has a corresponding *remedy*: when it fires, the operator runs the author worker, which usually produces a covering graph in one shot. The pair (gate + remedy) is balanced; neither alone would be tolerable.

**Negative / accepted trade-offs:**

- New friction on the existing `dec implement` path. Teams adopting decision-cli will, the first time, run `dec implement` and bounce off the gate. This is intentional friction (the chain is open; we want them to notice), but it must be a *good* error message ([FT-047](FT-047) carries that responsibility). A bad error here would be the worst outcome of this ADR.
- The waiver escape hatch will, in some teams, be over-used. The fitness-function corollary is the response: we measure and review the rate, not prevent any individual waiver.
- Environment-aware coverage is **not** enforced by this gate in slice 2.6. A feature can have a graph that covers all its TCs in env `ENV-001 (ephemeral-cli)` and the gate passes even if the work conceptually requires production-readonly verification too. This is deliberate: env strategy is the operator's call; the gate enforces "there is *some* coverage", not "the right coverage for this dispatch". Tightening this to per-env coverage is a slice-3 concern (likely an ADR amendment) once execution lands and we know what "right env" means in practice.
- The gate runs on every dispatch, including replay/rerun. There is no "skip gate on replay" option; if a replay would be ungated today, the team has a graph problem or a waiver gap, not a gate problem. Bypassing on replay would defeat the invariant.

## Forward references

- [FT-045](FT-045) — coverage check primitive.
- [FT-046](FT-046) — existing-graph matcher (used by [ADR-030](ADR-030)'s author worker; the gate uses only the coverage primitive directly).
- [FT-047](FT-047) — implementation of this gate inside the harness.
- Slice 3 — environment-aware coverage tightening; waiver-rate fitness function.
