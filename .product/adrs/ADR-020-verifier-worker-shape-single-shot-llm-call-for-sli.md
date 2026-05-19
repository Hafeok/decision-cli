---
id: ADR-020
title: 'Verifier worker shape: single-shot LLM call for slice 2'
status: accepted
features: []
supersedes: []
superseded-by: []
domains: []
scope: feature-specific
content-hash: sha256:8ccc04528970449ed6309622b27462ec167383888592308d1607d788b8ed7fa4
---

## Context

[ADR-008](ADR-008) defined the slice-1 worker contract: stateless, bundle-in / artifact-out, no graph access. The implementer worker ([FT-013](FT-013)) is a Python `code-writer` that takes a markdown bundle, calls Claude once with structured output, writes files, returns a `CodeChange`. The verifier worker for slice 2 needs the same contract.

The question for Phase A: does the verifier need *tool use* (e.g. ability to read additional files, run `cargo test`, query the graph) or is a single-shot LLM call sufficient?

## Decision

**Slice 2's verifier worker is a single-shot LLM call. No tool use, no follow-up reads, no graph access.**

The bundle the verifier receives is complete: produced artifact (`CodeChange` including the file diffs the action wrote), originating feature_spec, bundle hash that produced the action, TCs that validate the feature, cross-cutting ADRs. The verifier's job is to read these and emit a `VerificationVerdict` ([ADR-018](ADR-018)) with a rationale citing specific TCs/ADRs.

### Worker shape (Python)

Located under `workers/verifier/` (a new package, sibling to `workers/code-writer/`). Same SDP discipline as code-writer: stateless, no graph access, structured Pydantic input/output.

```python
class VerifierInput(BaseModel):
    feature_spec: str           # full feature_spec markdown
    produced_artifact: str      # the CodeChange or other action output as text
    bundle_hash: str            # for audit only
    relevant_tcs: list[TcRef]   # id, type, body
    relevant_adrs: list[AdrRef] # id, scope, body
    dispatch_iri: str           # for PROV-O linkage in the output

class VerificationVerdict(BaseModel):
    verdict: Literal["approved", "rejected", "amendment-required"]
    rationale: str              # ≥ 20 chars (ADR-018 SHACL minimum)
    violates: list[str] = []    # TC-XXX / ADR-XXX references (required if not approved)
    amendment_guidance: str | None = None  # required iff verdict == amendment-required
```

The worker runs the same Claude call shape as code-writer, but with the verifier system prompt and structured-output schema. Output is validated by Pydantic; the harness then maps it to RDF and writes it through `StreamWriter` ([ADR-005](ADR-005)) after SHACL passes.

### Why no tool use

- **Cold-context property.** [ADR-019](ADR-019) keeps action and interpretation sessions independent. Tool-using verifiers blur that line — once the verifier can run `cargo test`, the verifier's verdict depends on the workspace state at verification time, not on the artifact the action produced. We lose the property that interpretation is a function of evidence-in-the-bundle.
- **Auditability.** A single-shot verifier's full input is captured by the bundle hash. A tool-using verifier's behavior depends on the workspace at runtime, which makes replay non-deterministic.
- **Slice 2 scope.** Phase A is proving the loop closes. Tool use is a feature increment, not a correctness requirement.

### Why this specific shape (and not, e.g., a Rust binary)

The implementer worker is Python ([ADR-008](ADR-008)) and the verifier shares enough plumbing (Claude SDK, structured outputs, bundle parsing) that diverging languages here would duplicate that plumbing. Python is the worker language for Phase A; a future feature_spec can extract shared worker utilities into a `workers/_shared/` package as Phase B accumulates roles.

### What changes when this gets too small

A later ADR-020 amendment (or a successor ADR) records when verifier tool use becomes correct. Plausible triggers:

- The verifier needs to read files the action *didn't* touch (e.g. validate cross-cutting invariants) and bundling those files into every dispatch exceeds context window.
- Running `cargo test` programmatically gives meaningfully better verdicts than reading the diff (testable: compare verdict accuracy with vs. without test execution on a corpus of historical dispatches).
- A Phase C fitness function explicitly requires tool-call telemetry.

Until one of those fires, single-shot stays.

## Rejected alternatives

- **Tool-using verifier from day one.** Rejected — see "Why no tool use." Phase A wants the simplest observable shape.
- **Reuse the code-writer worker with a `--mode=verify` flag.** Rejected: muddles two worker contracts. The verifier's system prompt, output schema, and (eventually) model binding are different concerns; bolting them onto code-writer leaks responsibility.
- **In-process Rust verifier (no LLM at all).** Rejected: that's a deterministic check (e.g. running `cargo test`), not an interpretation. It's a different role — call it `test-runner` — and it composes with the verifier, not replaces it.

## Consequences

**Positive:**
- Two parallel worker packages with identical contracts → uniform harness code.
- Verifier behavior is fully determined by its prompt + the bundle content. Replay is deterministic.
- Cost per verification ≈ cost per implementation (similar context size for slice-2-scoped features).

**Negative / accepted costs:**
- The verifier cannot check things that aren't in the bundle. The bundle assembly logic must be correct.
- A new Python package to maintain.

**Enforcement:**
- TC asserting the verifier worker has no `graph_*` imports (the same convention `code-writer` follows under [ADR-008](ADR-008)).
- Pydantic strict-mode validation refuses unknown fields and missing required fields at worker exit.

## Status

Proposed. Bound to slice 2 ([FT-023](FT-023)). Inherits the slice-1 worker contract from [ADR-008](ADR-008).
