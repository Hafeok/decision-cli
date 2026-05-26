---
id: ADR-066
title: Bundle-completeness principle for graph-authoring workers
status: accepted
features:
- FT-101
- FT-102
- FT-103
- FT-104
supersedes: []
superseded-by: []
domains: []
scope: domain
content-hash: sha256:2f049cbf7498ae754387cb8521d5e60b6fa70cb4e0e1e2959d820541075aef65
---

## Context

When the verify-graph-author worker ([ADR-030](ADR-030), [FT-048](FT-048)) was asked to mint `dec:VerificationGraph` artifacts for features FT-097..FT-100 (the slice-3 graph runner), the worker produced four graphs that were SHACL-valid, satisfied the coverage check, and looked plausible at a glance — but inspection of the proposed steps showed them to be operationally inert:

- **Hallucinated namespaces.** Three of the four graphs used wrong `dec:` IRI bases (`https://decentralized-data.eu/`, `https://decisionframework.org/dec#`) instead of the real `https://decision-cli.dev/ns#`. Any SPARQL assertion authored against those bases will never match a triple in the actual store.
- **Hallucinated CLI invocations.** Steps invoked commands like `dec verify graph new --env ENV-001 --file <path>` — a flag combination that doesn't exist. The author had no structured reference for the real `dec` surface, so it inferred plausible-shaped commands from prose context.
- **Synthetic step bodies.** Multiple graphs contained `echo 'dummy graph content' > .dec/verify/graph.ttl` as a "verification step" — clearly a placeholder the LLM used to fill the coverage slot when it had no concrete suggestion.
- **Wrong query targets.** `sparql-assertion` steps targeted `.dec/store` (a directory, not a queryable endpoint) because no field in the bundle told the worker how to address the actual oxigraph store from a shell or HTTP context.

The first reaction is "the LLM is bad at this." The accurate reaction is **the LLM was authoring blind**. The author's input bundle ([FT-048](FT-048) §Inputs) contains: `feature_spec` body, the relevant TCs, one env record, candidate graphs, and the step vocabulary. It contains **nothing** about:

- which `dec` subcommands exist and what their observable effects are,
- the actual `dec:` namespace and the typed classes/predicates the worker can query for,
- how to address the orchestration store from inside a step (`dec sparql query --store ...`? `http://localhost:NNNN/sparql`?),
- what binaries, fixtures, and pre-seeded artifacts are present in the target env beyond `allowed_ops`,
- any concrete example of what a real, working verification graph looks like in this env.

Without these inputs, the LLM has two choices: fabricate plausible-looking content, or fail. Today it fabricates — and we ship SHACL-valid placeholders that satisfy `dec verify graph generate`'s coverage report while doing nothing of substance. The chain-integrity gate ([ADR-031](ADR-031)) is satisfied; the verification it nominally enables is fiction.

This is not a prompt-tuning problem and not an LLM-capability problem. It is a **bundle-shape problem**. The system's design intent — explicit in [ADR-008](ADR-008)'s "stateless bundle-in, artifact-out" — is that the worker is a pure function of its bundle. Whatever the LLM needs to author correctly *must be in the bundle*, full stop. When the bundle is incomplete, the worker's output is a function of the LLM's training data, not the system's actual state. That breaks the seam [ADR-008](ADR-008) deliberately set up: the worker becomes non-replayable (today's hallucination differs from tomorrow's), non-auditable (the rationale points at facts the worker invented, not at things in the bundle), and non-determinisable (two runs with the same bundle drift apart because the missing context fills in differently from the model's prior).

The same diagnosis applies, in principle, to any future graph-authoring worker: refactor-graph-author, deployment-graph-author, observation-graph-author. The principle below is not specific to verification — it is the worker contract's missing rule for the *graph-authoring* sub-class of workers.

## Decision

**A graph-authoring worker's input bundle must contain every piece of dec-specific knowledge the worker needs to author a correct, runnable graph. The worker is never expected to know dec facts — CLI surface, ontology vocabulary, store addressing, env capabilities, exemplar patterns — from prior knowledge. If the LLM needs to know X to author correctly, X is in the bundle.**

This decomposes into four operational rules:

### Rule 1 — Five categories must be carried in every graph-authoring bundle

Every graph-authoring worker's input contains, at minimum, these five fields in addition to the worker-specific payload (feature_spec, TCs, candidates, etc.):

1. **`cli_surface`** — a structured reference for every `dec` subcommand the worker may invoke from a `shell-command` step. Per command: the verb, the flags, the typed exit codes, and the *observable side effects* (which artifacts get written, which events get emitted, what stdout shape is produced). The worker reads this instead of inferring command shape from the feature_spec's prose.

2. **`ontology_vocabulary`** — the dec namespace as a literal IRI, plus the typed classes (`dec:Session`, `dec:VerificationGraphResult`, etc.) and the canonical predicate set per class. The worker reads this when composing SPARQL queries; an assertion against a namespace not listed here is a SHACL violation at the bundle layer, not at runtime.

3. **`store_query_surface`** — how to address the orchestration store from each step kind in each env type. For `ephemeral-tempdir`: the path of the local Turtle/N-Quads file or the in-process oxigraph endpoint. For `remote-http`: the SPARQL endpoint URL and authentication, if any. The worker reads this to set `dec:target` correctly.

4. **`env_capabilities`** — beyond the abstract `allowed_ops` tokens, concrete facts about the env: which binaries are on PATH, the working-directory lifecycle (fresh-per-run vs. persistent), pre-seeded artifacts (e.g. an `ephemeral-tempdir` env may seed a baseline `.dec/store` from a fixture), available environment variables. The worker reads this to know what it can rely on without writing setup steps.

5. **`exemplar_graphs`** — a small, curated set of known-good `dec:VerificationGraph` artifacts that have been validated to work in the env's `safety_class`. Three to five per env type is enough. The worker uses these as pattern-match templates ("a real verification graph in `ephemeral-cli` looks like this; mine should look similar in shape").

The five fields are normative on the bundle shape, not on the worker's prompt — they are present even when a particular dispatch does not use them.

### Rule 2 — The bundle composition is itself queryable

The five fields are populated by the bundle assembler from **first-class artifacts in the store**, not from hardcoded literals in the assembler's source code. The assembler runs SPARQL `CONSTRUCT` queries against the orchestration store to pull the current CLI surface, the active ontology, the registered exemplars, the env's capability declaration; their content-hashes are recorded on the resulting bundle.

This is the [ADR-002](ADR-002) graph-as-state stance applied consistently. The CLI surface is not "what the assembler thinks `dec` looks like at the time it was compiled"; it is "what the orchestration store records that `dec` looks like, queryable like every other fact." When `dec` grows a new subcommand, the CLI surface artifact is updated; the next bundle automatically carries the new surface; the worker authors correctly without a deploy of the bundle assembler.

[FT-102](FT-102) defines the three artifact types that back this rule: `dec:CapabilityReference`, `dec:OntologyDescription`, `dec:ExemplarGraph`. The env capability declaration extends `dec:VerificationEnvironment` ([FT-035](FT-035)) with an optional `dec:concreteCapabilities` block rather than introducing a separate type.

### Rule 3 — Missing context is a system error, not a worker failure

When a graph-authoring dispatch produces a graph whose steps reference facts not present in the bundle (a SPARQL query against a namespace not in `ontology_vocabulary`, a `dec` command not in `cli_surface`, an env binary not in `env_capabilities`), the orchestrator **refuses to persist the proposal** and emits a `dec:Feedback` with `class = "gap"` targeted at the bundle assembler, not at the worker.

The gap class is the existing controlled vocabulary entry ([ADR-023](ADR-023)) for "the upstream artifact is missing context the downstream needed." Applying it here turns the silent-fabrication failure mode visible: the operator sees "the bundle was incomplete" rather than "the LLM hallucinated." The remedy is to extend the relevant artifact (add the missing command to `CapabilityReference`, add the missing predicate to `OntologyDescription`, add a covering exemplar), not to re-prompt the worker.

This rule applies symmetrically to the verifier role's run-time check: a `shell-command` whose verb is not in `cli_surface`, or a `sparql-assertion` whose target IRI's namespace is not in `ontology_vocabulary`, is `unrunnable` with the same `gap`-class feedback, not `fail`. The distinction matters for fitness-function reporting ([ADR-014](ADR-014)): "the system was misverified" is a different signal from "the verification worked and the code was wrong."

### Rule 4 — Bundle-completeness validation runs at dispatch time, not authoring time

The validation in Rule 3 is performed at **dispatch time** by the orchestrator, after the worker returns and before the proposal is persisted. It is not the worker's responsibility to self-check (which would defeat the seam — the worker's output is data, not a SHACL-validating actor) and it is not the bundle assembler's responsibility (which would require it to know what the worker chose to do, which it cannot until the worker has done it).

The validator is a SPARQL query that, given a proposed graph and the bundle's `cli_surface` / `ontology_vocabulary`, returns the set of `(step, referenced_thing)` tuples that reference things outside the bundle. Non-empty result → reject. The query is itself a `dec:QueryTemplate` artifact ([ADR-043](ADR-043) §full-chain traversal pattern) so it is versioned and replayable.

## Rejected alternatives

### Teach the LLM more in the prompt

Some of the missing context — dec namespace, the six step kinds — could be inlined as a fixed prompt prefix. Rejected: this conflates "stable facts about the system" with "facts about this specific dispatch's bundle", undoes the [ADR-008](ADR-008) seam (the worker is no longer a pure function of its bundle), and turns every dec evolution into a worker-prompt rev. Putting the same content in the bundle is the same payload size with the right architectural shape.

### Train a specialised model on the dec API

Out of slice scope and architecturally wrong for the same reason as the prompt-prefix option: the worker would carry implicit knowledge that the bundle does not. Replay would no longer be reproducible from `(bundle, model_id)` alone — it would depend on which model version was trained when, which is unauditable.

### Validate against the bundle inside the worker

Self-check inside the worker would let it refuse to emit a hallucinated proposal. Rejected: the worker contract is "input → artifact" — adding a self-validation phase introduces a second internal state the orchestrator cannot inspect. The orchestrator's chokepoint validator (Rule 4) catches the same violations from outside and emits structured feedback the system can route.

### Hardcode the bundle assembler

Today's bundle assembler already hardcodes some choices (which TCs to pull, how to compute `bundle_hash`). Continuing in that style for the five new fields would mean: every change to the dec CLI requires editing the assembler. Rejected — the [ADR-002](ADR-002) stance is that the source of truth for "what dec looks like" is the orchestration store, not the assembler's source code. [FT-102](FT-102) carries the artifact types that let the assembler stay declarative.

### Couple verification graphs to product-cli TC runners

A tempting shortcut: when a TC frontmatter declares `runner: cargo-test`, the verify-graph-author emits a single `shell-command` step `cargo test ...`. Rejected: this would make dec verification a thin wrapper around product-cli's runners — which is fine for code-shaped TCs but blocks the design goal of dec verification reasoning about *system-level* properties (deployment, observability, multi-step procedures) that cargo cannot express. The two pipelines stay parallel by design; this ADR makes the dec pipeline self-sufficient rather than absorbing into the other.

## Consequences

### Positive

- The worker becomes a true pure function of its bundle, restoring the [ADR-008](ADR-008) seam at the level it was meant to operate.
- Hallucination becomes structurally impossible — any step that references something outside the bundle is rejected by the chokepoint validator. The proposal either uses the bundled facts or the dispatch fails loudly with structured feedback.
- The system's state is queryable for "what does dec look like right now": the `CapabilityReference` artifact is the source of truth, queryable like any other fact.
- The bundle-completeness validator gives operators a single dashboard for "where is dec under-described to its own workers" — a `gap`-class feedback rate from the bundle assembler is the metric.
- The same five-field shape will apply to every future graph-authoring worker. Refactor-graph-author, deployment-graph-author, etc. inherit the rule without re-deciding it.

### Negative / accepted trade-offs

- Bundle size grows from ~5 KB to ~20–50 KB per dispatch. This is a one-time per-dispatch cost and well within current model context windows; bundle bytes do not dominate cost or latency at the dispatch rates the system is designed for.
- Three new artifact types ([FT-102](FT-102)) add to the dec ontology surface. Each needs a SHACL shape, CLI authoring verbs, and migration handling for envs that exist before the new fields are populated (a graceful-default story is in [FT-101](FT-101)).
- The bundle assembler grows a SPARQL-pulling responsibility for each of the five fields. The complexity is contained — each field has one query — but it raises the bar on assembler tests.
- The chokepoint validator at dispatch time adds latency to every graph-authoring dispatch (one SPARQL pass over the proposal). The pass is bounded by step count and is cheap in absolute terms; the ergonomic cost of a rejected-proposal cycle is the real trade-off, and the answer is to make the gap-feedback actionable enough that the cycle is short.
- Until [FT-101](FT-101) and [FT-102](FT-102) land, the existing FT-097..FT-100 verification graphs minted under the old bundle shape are placeholders that the slice-3 runner will report as `unrunnable`. The migration path is to regenerate them once the enriched bundle is available; the chain-integrity gate's waiver mechanism ([ADR-031](ADR-031)) covers the interim.

## Forward references

- [FT-101](FT-101) — verify-graph-author bundle enrichment (extends `VerifyGraphAuthorInput` with the five fields; adds the chokepoint validator).
- [FT-102](FT-102) — first-class artifact types for `CapabilityReference`, `OntologyDescription`, `ExemplarGraph` (the substrate the assembler queries).
- Future — refactor-graph-author and other graph-authoring roles inherit the same five-field bundle shape.

## Status

Proposed. Bound to slice 3+; the verifier role's runtime validation under Rule 3 is the partner change once the slice-3 runner ([FT-098](FT-098)) lands.
