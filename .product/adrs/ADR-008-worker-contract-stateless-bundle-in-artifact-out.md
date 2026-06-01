---
id: ADR-008
title: 'Worker contract: stateless bundle-in, artifact-out'
status: accepted
features:
- FT-059
- FT-060
- FT-064
- FT-102
supersedes: []
superseded-by: []
domains: []
scope: feature-specific
content-hash: sha256:0c18aa723a8f51db3e8ceecd5702680f103d27145432f5026950cae1df2cfb45
amendments:
- date: 2026-05-22T18:27:36Z
  reason: Capability layer ([ADR-033](ADR-033)) injects (endpoint, model_identifier, parameters) into the dispatch payload alongside the bundle; record the payload extension so the worker contract documentation matches the reality the dispatcher produces after [FT-061](FT-061) and [FT-062](FT-062). Worker statelessness and bundle-completeness are preserved — this is a payload-shape clarification, not a contract change.
  previous-hash: sha256:ad73cf6f1d820f25ff99a40fe0e514ba852bd2ff77b40a4ed8550b98ccfb6bea
- date: 2026-06-01T18:47:00Z
  reason: Role-scoped tool surfaces ([ADR-070](ADR-070)) introduce `allowed_tools` to the dispatch payload alongside the capability triple recorded by the Phase-2 amendment. The in-process LiteLLM-client agentic loop ([ADR-069](ADR-069), [FT-123](FT-123)) consumes the field; the field is sourced from the role catalog by the dispatcher. Worker statelessness, no-graph-access, and bundle-completeness invariants are unchanged — this is a payload-shape extension, not a contract change. Same shape as the Phase-2 amendment for [ADR-033](ADR-033).
  previous-hash: sha256:f4a8e87cbf0d8a7e6089ebec78f38854b4ebbe2bbcdc7ecaf9cef4d9a620eb76
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

### Amendment — capability injection (Phase 2, post [ADR-033](ADR-033))

The dispatch payload is extended by [FT-061](FT-061) to include the dispatcher-resolved capability triple alongside the bundle:

- `endpoint` — `"scaleway" | "anthropic"`.
- `model_identifier` — the exact provider model string.
- `parameters` — temperature, max_tokens, optional `reasoning_effort`, optional tool definitions, optional response schema.
- `capability_ref` and `binding_ref` — version pins recorded on the session for reproducibility.

Workers consume the triple verbatim via the `ModelRouter` from [FT-060](FT-060); they remain ignorant of capabilities, role bindings, escalation, and cost. The statelessness, no-graph-access, and bundle-completeness invariants are unchanged — the payload simply grew. See [ADR-033](ADR-033) for the rationale and the cleanly orthogonal roles of `dec:WorkerBinding` (which executable) and `dec:Capability`/`dec:RoleBinding` (which model).

### Amendment — tool-surface injection (Phase 3, post [ADR-070](ADR-070))

The dispatch payload extends further to include the role-scoped tool surface resolved from the role catalog:

- `allowed_tools` — `list[str]`. Short snake_case tool names (`read_file`, `write_file`, `run_build`, `run_lint`, `run_tests`, …). Sourced from `dec:roleTool` quads on the dispatch's role per [ADR-070](ADR-070). Empty list means no tools granted; the worker fail-closes with `WorkerError(category="invalid_dispatch")` before the first LLM call, per [ADR-069](ADR-069).

The in-process LiteLLM-client agentic loop ([ADR-069](ADR-069), [FT-123](FT-123)) intersects `allowed_tools` with its own tool registry and exposes only the intersection to the model. Path containment and secrets blocking on every tool call are enforced per [ADR-071](ADR-071).

Workers remain ignorant of *where* the tool surface comes from (role catalog, SHACL shape, dispatcher logic) — they consume the resolved list verbatim. The statelessness, no-graph-access, and bundle-completeness invariants are unchanged. The payload shape after Phase 3 is `(bundle, endpoint, model_identifier, parameters, capability_ref, binding_ref, allowed_tools)`.

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

Accepted. Governs FT-013 (code-writer worker) and FT-011 (harness assembles bundle, writes artifacts on worker's behalf). Future workers in later slices inherit this contract. Phase-2 capability injection extension recorded by amendment, governed by [ADR-033](ADR-033). Phase-3 tool-surface injection extension recorded by amendment, governed by [ADR-069](ADR-069) and [ADR-070](ADR-070).