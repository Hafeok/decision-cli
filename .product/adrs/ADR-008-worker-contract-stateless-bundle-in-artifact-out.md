---
id: ADR-008
title: 'Worker contract: stateless bundle-in, artifact-out'
status: accepted
features: []
supersedes: []
superseded-by: []
domains: []
scope: feature-specific
content-hash: sha256:ad73cf6f1d820f25ff99a40fe0e514ba852bd2ff77b40a4ed8550b98ccfb6bea
---

## Context

Workers execute roles: an `implementer` worker runs the implementer role, a `reviewer` worker would run the reviewer role, and so on. The worker contract shapes how the entire orchestration loop behaves.

Three contracts could be considered:

1. **Stateful workers.** Workers hold connections to the graph, mutate it directly, manage their own session state.
2. **Pipeline workers.** Workers receive a stream of inputs and produce a stream of outputs; the orchestrator manages plumbing.
3. **Stateless function workers.** Each invocation is `bundle → artifact`. Workers receive everything they need in the bundle and return a single structured output.

Stateful workers couple workers to graph internals — they would need their own SPARQL clients, write tools, error handling for graph integrity. Pipeline workers complicate the audit story (where does a session begin and end?). Stateless workers match the DDD framing exactly: every role consumes a deterministic bundle and produces a typed artifact; roles never talk to each other.

See `decision-cli-slice-1-bounds.md` §7.

## Decision

Workers are **stateless functions** with shape `bundle → artifact`. The contract is intentionally narrow:

- Workers receive a serialised bundle (markdown for slice 1) via the dispatch event payload.
- Workers **do not talk to the graph**. The harness assembles bundles and writes artifacts on the worker's behalf.
- Workers produce a structured output conforming to the role's output schema (Pydantic in Python; validated against SHACL by the harness on write).
- Workers report session telemetry: tokens, latency, tool-call history, errors.
- Workers may emit **feedback artifacts** during execution — deferred to slice 2. Slice 1 workers report errors but do not emit structured feedback.

The Python code-writer worker for slice 1 receives a bundle for an implementer session, calls Claude with structured output, and returns a `CodeChange` artifact describing files written and a diff summary. The worker writes files directly to the configured workspace directory; the harness records the session and writes the `CodeChange` to product-cli's graph via MCP.

## Consequences

**Positive:**

- Workers are simple to write, simple to test (`bundle in, artifact out`), and easy to swap.
- The harness is the **only** graph mutator. All audit / scope / integrity logic lives in one place.
- The worker contract is uniform across languages — a Python worker, a Rust worker, and a shell worker all conform to the same shape.
- Workers can be developed and tested in isolation against canned bundles.

**Negative / accepted costs:**

- The bundle must contain everything the worker needs. Bundles get larger; bundle-assembly logic gets more careful.
- A worker that wants graph data not in the bundle has no escape hatch. **This is intended.** If the bundle is missing context, that is a smell: the bundle assembly should be fixed, not the worker should be given graph access.

**Boundary enforcement:**

- Reviewers must reject any worker code that imports an Oxigraph client or makes SPARQL calls.
- If a worker needs something not in the bundle, the response is "fix the bundle," not "give the worker graph access."

## Status

Accepted. Governs FT-013 (code-writer worker) and FT-011 (harness assembles bundle, writes artifacts on worker's behalf). Future workers in later slices inherit this contract.
