---
id: ADR-070
title: Role-scoped tool surfaces declared in the role catalog
status: proposed
features:
- FT-126
- FT-127
- FT-128
- FT-129
- FT-130
- FT-132
- FT-133
supersedes: []
superseded-by: []
domains: []
scope: cross-cutting
---

## Context

[ADR-069](ADR-069) replaces `claude -p` with an in-process LiteLLM-client agentic loop in the code-writer worker. That decision raises the question this ADR settles: **where does the list of allowed tools for a dispatch come from?**

Three candidate sources:

1. **The feature_spec.** Each FT declares the tools its implementer needs.
2. **The worker's hardcoded default.** The Python module declares one canonical set; every dispatch gets the same.
3. **The role catalog.** Each `dec:Role` declares its `dec:roleTool` set; the dispatcher resolves the role, reads the surface from the catalog, and threads it through the payload.

(1) ties a runtime concern (which tools to expose) to a spec-time artifact (what behaviour to ship). It also doesn't scale to roles that don't have a 1:1 feature mapping — verifier, reviewer, summariser. Every spec would have to repeat the same `allowed_tools: [...]` block.

(2) is what the codebase does today, implicitly: `_subprocess_runner.py` runs `claude -p` with no `--allowed-tools` argument, getting the Claude Code default surface. The `DispatchPayload.allowed_tools` field exists in `models.py:87` but is read by no code. A hardcoded default is fine when there is one worker and one role; it breaks the moment a verifier worker needs to run without `write_file`.

(3) decouples the tool surface from both spec authoring and worker source. The role *is* the dispatch atom — [ADR-008](ADR-008) and [ADR-033](ADR-033) already establish that the dispatcher resolves `(role, capability) → (endpoint, model_id, parameters)`. Adding `allowed_tools` to the resolved tuple is a natural extension of the same lookup: the role catalog tells the dispatcher *what* to call and *with which tools*.

The pipeline factory enforces a similar principle through external machinery: `pipeline.yaml` declares `mcp_servers: [...]` per step, a JWT step token carries the `allowed_servers` claim, and the MCP runtime refuses unallowed servers. We borrow the principle — tool surface is a property of the role/step, not the worker — but encode it in the graph rather than in YAML and JWTs, because `dec` is graph-native and in-process.

There is a related concern this ADR does **not** settle: how to model finer-grained scoping within a tool (e.g. "may write only `src/**`" vs. "may write `**/*`"). The role catalog declares tool *identities*; per-call constraints live in the tool implementation (workspace containment per [ADR-071](ADR-071)). Sub-resource scoping is deferred until a feature_spec demands it.

## Decision

**Source of truth for a dispatch's tool surface is the role catalog.** Every `dec:Role` in the orchestration store declares one or more `dec:roleTool` literals naming a tool the role is allowed to invoke. SHACL refuses any `dec:Role` quad-set with `sh:minCount 1` on `dec:roleTool`.

Concrete substance:

1. **Vocabulary.** Add the predicate `dec:roleTool` (literal range — short snake_case tool name). The seed catalog provides the implementer role with `["read_file", "write_file", "run_build", "run_lint", "run_tests"]` and the verifier role with `["read_file", "run_build", "run_lint", "run_tests"]` (no `write_file`).
2. **SHACL.** Extend the existing role shape to require `dec:roleTool` minCount 1 on every `dec:Role` instance. New roles MUST declare a surface; the catalog refuses an unscoped role.
3. **Dispatcher path.** The Rust `build_dispatch_payload()` in `features/implement/lifecycle.rs:25-52` extends to read `allowed_tools` from `role_catalog::lookup(&store, &role_iri)` and write it into `DispatchPayloadJson`. The lookup returns `Vec<String>` — empty for legacy stores that pre-date this ADR (grandfathered per [ADR-042](ADR-042); the worker fail-closes per [ADR-069](ADR-069)).
4. **Worker contract.** The worker treats `payload.allowed_tools` as the deny-list complement of its own tool registry: any tool present in the registry but absent from `allowed_tools` MUST NOT be exposed to the model. Empty intersection → `WorkerError(category="invalid_dispatch", message="no tools granted")` before the first LLM call.
5. **Future roles.** Any new role (reviewer, summariser, planner, …) lands its catalog seed with a `dec:roleTool` list at the same time as its capability binding. There is no "default tool set" the worker falls back to.

This ADR is **cross-cutting** because every future worker, every future dispatch payload, and every future role bundle inherits the rule. The TC that validates the SHACL constraint surfaces in every implementation session's context bundle; new roles cannot quietly ship without declaring their surface.

## Consequences

**Positive:**

- A reviewer role with no `write_file` is a catalog edit, not a Python `if role == ...`. Adding role-specific tool surfaces becomes a pure graph operation.
- Tool surface is auditable from the graph: `dec session show <id>` can join the session's role to its `dec:roleTool` set and explain why a particular tool was (un)available.
- The decision is portable: a Rust worker, a Python worker, and a shell worker all read the same field and enforce the same set. Multi-language uniformity falls out for free.
- Fail-closed semantics line up with the broader SDP boundary: a worker that sees an empty `allowed_tools` refuses to dispatch, rather than silently widening to "all tools".

**Negative / accepted costs:**

- Adding a tool means *two* edits: a Python tool implementation in the worker AND a role-catalog seed update so the role can call it. The two-step is intentional — it prevents new tools from being silently exposed to every role.
- Legacy stores authored before this ADR have no `dec:roleTool` quads. Per [ADR-042](ADR-042) we grandfather them: lookups return an empty Vec, the worker refuses the dispatch with a structured error, the operator must re-seed the catalog or migrate via `dec init`. We do not retroactively rewrite legacy stores.
- The dispatch payload grows by one field. The serde defaults absorb the wire-format change for old workers (they see no field); for new workers reading old payloads, the empty default + fail-closed behaviour makes the failure mode loud and immediate.

**Boundary enforcement (cross-cutting fitness gate):**

A TC linked to this ADR runs SHACL validation against every seeded role in the catalog and asserts that each carries a non-empty `dec:roleTool` set. `product verify --platform` runs this TC; a PR that adds a role without declaring its tool surface fails the gate.

## Alternatives considered

- **Feature_spec declares allowed_tools per FT.** Rejected: ties runtime concerns to spec-time artifacts, doesn't compose for roles without 1:1 feature mapping, forces every FT to repeat the same list.
- **Worker module declares hardcoded default.** Rejected: works for one worker / one role; breaks the moment a second role needs a different surface (e.g. verifier without `write_file`). Also breaks the cross-language uniformity property.
- **Per-dispatch CLI flag (`dec implement --tools=...`).** Rejected: tool surface should be a property of the role, not of an invocation. The role catalog gives us this for free; a CLI flag would let a careless operator widen the surface accidentally.
- **External policy engine (OPA, Cedar) consulted on every dispatch.** Rejected for slice 1 as massive over-engineering for what is a per-row catalog read. If we ever need conditional policies ("verifier may use write_file when the bundle includes a fix-up flag"), an engine becomes reasonable; not before.
- **Sub-resource scoping in the catalog (e.g. `write_file: ["src/**"]`).** Out of scope. The catalog declares tool identities; per-call constraints live in the tool implementation. This ADR's vocabulary leaves the door open for a later `dec:roleToolScope` predicate but does not require it.

## Status

Proposed. Once accepted, governs the FT-121 implementation (Rust catalog plumbing + SHACL + seed quads). [ADR-069](ADR-069) consumes the resulting `allowed_tools` field via the dispatch payload; [ADR-071](ADR-071) governs the in-process containment that the named tools execute under.
