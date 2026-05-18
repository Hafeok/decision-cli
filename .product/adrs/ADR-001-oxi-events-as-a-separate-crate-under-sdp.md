---
id: ADR-001
title: oxi-events as a separate crate under SDP
status: accepted
features: []
supersedes: []
superseded-by: []
domains: []
scope: cross-cutting
content-hash: sha256:f21fbad45664668bda9603d6458c9cde2fa18447a30dc7be500fccfc5031ce45
---

## Context

decision-cli needs a graph-native event substrate: a mutation chokepoint, a subscription registry, evaluator, event emission with provenance, an outbox, delivery transports (in-process + SSE), and replay. The architectural question is whether this substrate is internal to decision-cli or extracted as an independent crate.

The Decision-Driven Design framing distinguishes the *framework* (generic substrate, vocabulary of mutations / subscriptions / events / delivery) from the *application* (DDD vocabulary: roles, bundles, sessions, policies, autonomy levels). Mixing the two couples the framework's evolution to one application's needs and prevents reuse.

See `decision-cli-slice-1-bounds.md` §5.1, §5.2 and `CLAUDE.md` "The line that must not be crossed."

## Decision

Extract the event substrate as a separate, independently-versioned Rust crate: **`oxi-events`**.

Apply the Stable Dependency Principle:

- `oxi-events` depends only on substrates **more stable than itself**: `oxigraph`, `tokio`, `tokio-stream`, `axum`, `serde`, `tracing`.
- `oxi-events` has **no dependency on `decision-cli`** and **no awareness of DDD-specific concepts** (roles, bundles, sessions, policies).
- The framework's public vocabulary is strictly *mutations, subscriptions, events, delivery*. Everything else is application territory.

The crate lives inside decision-cli's workspace for slice 1. Separate-repo extraction is deferred until the API has been pressure-tested by more than one consumer.

## Consequences

**Positive:**

- The framework is reusable. A second consumer can adopt `oxi-events` without inheriting DDD vocabulary.
- decision-cli can evolve its DDD vocabulary independently without churning the substrate.
- The boundary is enforceable as a build-time invariant (cargo `[dependencies]` and absent `use` statements).
- The crate is the natural unit for community contribution if a wider audience emerges.

**Negative / accepted costs:**

- Slightly more friction adding cross-cutting features that would otherwise span both layers — they must be expressed in framework vocabulary first.
- An additional crate in the workspace to publish and version.

**Enforcement:**

- Reviewers must reject any `use decision_cli::…` or DDD-vocabulary identifier appearing in `crates/oxi-events/`.
- If a feature_spec asks for something in `oxi-events` that requires DDD vocabulary, that is a smell: the feature belongs in `decision-cli`, with `oxi-events` providing only the generic substrate.

## Status

Accepted. Governs FT-001..FT-005 and the crate boundary in `crates/oxi-events/`.
