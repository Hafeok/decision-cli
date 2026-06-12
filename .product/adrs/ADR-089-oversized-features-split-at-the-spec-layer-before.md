---
id: ADR-089
title: Oversized features split at the spec layer before implementation dispatch; the drive planner gates on deterministic size signals
status: accepted
features: []
supersedes: []
superseded-by: []
domains:
- api
- data-model
scope: domain
content-hash: sha256:9965ac8033052fd922c8d0056c79934b56b407aa4e1b94d0fd2a091c90da1c7f
---

**Status:** Proposed

## Context

decision-cli has no answer to an oversized feature. The system's current responses to "this spec is too big for one dispatch" are all degradations:

- **Truncation.** [FT-163](FT-163) caps the per-cell framing of the feature_spec body at 50k chars — the worker is shown *less of the spec* rather than given less work. Information loss at exactly the moment a large feature needs its full context most.
- **Turn exhaustion.** The broad code-writer runs into the `max_turns` cost net (40 for cluster cells, [FT-164](FT-164)) and fails late, after spending the tokens.
- **Escalation burn.** The escalation chain ([FT-062](FT-062)) interprets "too big" as "model too weak" and retries the same oversized bundle on more expensive tiers.

The Definition-of-Ready planner ([FT-119](FT-119), [ADR-075](ADR-075), [ADR-079](ADR-079)) gates dispatch on spec *completeness* — body sections, linked TCs, preflight acknowledgements — but never on spec *size*. A complete 40k-char feature touching five crates sails through DoR into a dispatch that cannot succeed in one session.

The pipeline factory treats batch size as a first-class gate (its ADR-040, DORA's small-batch accelerator): a dedicated cheap-model `split` step (S0.5) decomposes any goal exceeding declared batch limits into sub-goals *before* the expensive steps run, and the build gate independently enforces PR size (≤ 800 changed lines). The principle to borrow is **decompose before dispatch, on declared limits**. The factory's mechanics do not transfer directly, though: its sub-goals are ephemeral runtime objects living only in the run context. decision-cli's prime directive is that engineering work is authored through the spec layer first — a unit of implementation with no feature_spec has no TCs, no `product verify` gate, no provenance, and no place in the dependency order. Runtime-only sub-goals would be invisible to everything the graph guarantees.

The system already has the machinery a spec-layer split needs: graph-authoring workers exist (verify-graph-author; bundle-completeness per [ADR-066](ADR-066)), acceptance autonomy for authored artifacts is settled per artifact kind ([ADR-075](ADR-075)), and `dec product` exposes the authoring surface ([FT-105](FT-105)).

## Decision

**An oversized feature is not dispatchable. The drive planner gates on deterministic size signals before implementation dispatch, and the remedy is decomposition at the spec layer: sub-feature_specs authored in the product graph, ordered by `depends-on`, each independently passing DoR. Splitting is a role; accepting a split is governed by ADR-075.**

1. **Deterministic size gate in the drive chain.** The DoR/ship planner computes size signals that are pure functions of the graph: spec body length, count of functional-spec subsections, linked-TC count, and declared touched-area breadth (domains). Signals exceeding thresholds make the planner return `split-required` (a Stuck variant carrying the measured signals) instead of dispatching. No LLM is consulted to *detect* oversize.
2. **Thresholds are graph-resident policy with compiled defaults.** Limits live in the orchestration store as a policy artifact (consistent with catalogs-in-graph, [ADR-036](ADR-036)); compiled defaults apply for legacy stores. Defaults are set conservatively below the FT-163 truncation cap — a spec that would be truncated for a cluster cell must have hit `split-required` first.
3. **The split itself is a dispatched role.** A `splitter` role (graph-authoring worker under [ADR-066](ADR-066)) receives the parent spec and its dependency context and proposes sub-feature_specs: each with its own body, TC stubs, `depends-on` edges among siblings, and a link back to the parent. The proposal is staged for acceptance per [ADR-075](ADR-075)'s artifact-kind autonomy — spec artifacts require operator acceptance until graduation.
4. **The parent becomes an umbrella.** The parent feature is not deleted: it keeps its identity, links its children, and completes when its children complete. Derivation provenance (split-from, plus the signals that triggered the split) is recorded per the dual-provenance rule ([ADR-038](ADR-038)).
5. **Sub-features re-enter the normal pipeline.** Each child passes DoR, preflight, and `product verify` independently; the dependency order makes `dec drive ship --all` sweep them in sequence. Splitting composes with, and does not bypass, every existing gate.

## Rationale

- **Decompose-before-dispatch beats every current failure mode.** Truncation loses information; turn caps and escalation burn tokens proving a bundle was too big. The gate is free (pure graph reads) and fires before any spend.
- **The spec layer is where decomposition survives.** Sub-features get TCs, verify gates, provenance, and dependency ordering for free — the entire reason the spec-first principle exists. The factory's runtime split is the right idea attached to the wrong (for us) layer.
- **Deterministic detection, LLM-authored remedy.** Detection must be a gate, so it must be deterministic and cheap. The split itself is judgment work (where to cut, what each child owns), which is exactly what a role with a curated bundle is for.
- **DORA's small-batch evidence transfers.** Smaller features mean shorter dispatch sessions, tighter audit loops, more precise repair targeting ([ADR-087](ADR-087)), and cheaper escalation — the same accelerator logic the factory builds on.

## Rejected alternatives

- **Runtime goal decomposition (the factory's literal shape).** Ephemeral sub-goals have no feature identity: no TCs bind to them, `product verify` cannot gate them, provenance has no subject, and a failed sub-goal leaves no graph trace. Violates the spec-first principle outright.
- **Raise context windows / keep truncating.** Truncation already shipped one unpromotable cluster (FT-147 lessons); larger windows move the cliff without removing it and inflate per-dispatch cost for every feature, sized or not.
- **Human-only splitting discipline.** Works at today's scale, dies with `dec drive ship --all` autonomy — the headless sweep is precisely where an unsplittable oversized feature stalls a whole run.
- **LLM-judged size detection.** A nondeterministic gate cannot be a gate: the same feature would dispatch on Monday and split on Tuesday. Deterministic signals with declared thresholds keep the gate auditable; the LLM's judgment is reserved for the split proposal.
- **Splitting inside cluster dispatch (more cells instead of more features).** Cells are typed by TaskType, not sized by spec; an oversized feature usually exceeds the *TaskType's* scope, not a cell's. Cell-level slicing would push spec-shaped work into the cell catalog where no verify gate exists.

## Test coverage

- A feature exceeding a size threshold makes the planner return `split-required` with the measured signals; no dispatch occurs.
- A feature under all thresholds dispatches unchanged.
- Thresholds resolve from the graph policy artifact, with compiled defaults on a legacy store.
- An accepted split records derivation provenance and sibling `depends-on` edges, and each child independently passes DoR.
