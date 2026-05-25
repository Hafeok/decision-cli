# Working session: pipeline-worker slice 1

Authoring format: option-2 (one file per working session, typed sections per
artifact, `@<predicate> <ref>` for edges). Predicate names and ID conventions
are proposals — adjust to whatever product-cli's catalog actually uses.

---

## Brief brief:pipeline-worker-slice-1

title: Build pipeline-worker slice 1 — the worker SDK for pipeline-cli

@references doc:impl-doc-§5    (worker contract)
@references doc:impl-doc-§7    (event substrate)
@references doc:impl-doc-§11   (slice 1 strategy)
@references doc:entity-reference-feedback
@references doc:foundations-sensing
@references brief:dual-provenance-discipline    (sibling Brief, not yet authored)

@decomposes_into feature:brief-artifact-type
@decomposes_into feature:wire-layer
@decomposes_into feature:session-layer
@decomposes_into feature:bundle-layer
@decomposes_into feature:artifact-layer
@decomposes_into feature:provider-abstraction
@decomposes_into feature:side-channel
@decomposes_into feature:driver-interface
@decomposes_into feature:event-driver
@decomposes_into feature:shape-codegen

@excludes feature:custom-provider-adapters   (LiteLLM handles all providers; we don't write per-provider SDK wrappers)
@excludes feature:replay-driver-impl         (slice 2)
@excludes feature:tool-use-lineage           (slice 3, paired with implementer role)
@excludes feature:multi-process-worker-pool  (slice 4+)
@excludes feature:token-rotation             (operational, deferred)
@excludes feature:rust-counterpart-sdk       (only when code-shaped action worker demands it)
@excludes feature:meta-loop-eval-workflow    (depends on replay-driver-impl)

@acknowledges ack:security-deferred
@acknowledges ack:full-provenance-discipline-separate

premise:
  pipeline-cli (the Rust harness) is being built per the impl doc, with slice 1
  pushing one artifact end-to-end through one role. The worker side of that flow
  — the Python process the harness dispatches to — has no SDK. Without one, each
  worker reinvents the contract: bundle deserialization, artifact emission with
  SHACL validation, telemetry capture, side-channel feedback and judgment
  emission, provider abstraction, capability-tag dispatch. Reinventing it per
  worker is the failure mode this Brief prevents.

  Provider abstraction specifically is delegated to LiteLLM rather than built
  from scratch (per adr:litellm-as-provider-substrate). LiteLLM is the proxy
  workers call for every LLM completion; pipeline-cli's worker-distribution
  Brief ships LiteLLM as a slice-1 service. The SDK's Provider layer is a thin
  LiteLLM client, not a multi-provider abstraction layer.

goal:
  Ship a Python library that encodes the worker contract from impl doc §5 once,
  used by every Python worker pipeline-cli ever dispatches to. Slice 1 of the
  SDK demonstrates the contract end-to-end against pipeline-cli's slice 1:
  one dispatch event consumed over SSE, one bundle materialized as an in-memory
  pyoxigraph sub-graph, one LLM call routed through LiteLLM (which routes to
  Anthropic in slice 1 but could route anywhere), one artifact built and
  SHACL-validated, one completion event posted back. The replay driver
  interface is scaffolded but not implemented.

success_criteria:
  - A single Python worker subscribes to pipeline-cli's dispatch endpoint and
    receives a dispatch event for its advertised capability tag.
  - The worker queries the bundle via codegen'd accessors (no hand-written
    SPARQL), calls an LLM via LiteLLM with structured output (instructor on
    top of LiteLLM), builds an artifact via the codegen'd builder, and commits
    it.
  - LiteLLM's logging callback posts call telemetry (tokens, latency, cost,
    model, provider) to pipeline-cli; the session record reconciles it.
  - The completion POST returns 200 from pipeline-cli, with the artifact triples
    written through GraphWriter into the session's named graph and PROV-O
    annotations attached.
  - Emergent judgments and feedback emit triples into the same completion
    payload, routed through the side-channel layer.
  - The same worker code runs under the replay driver interface (interface only;
    concrete ReplayDriver is slice 2 work).

---

## Feature feature:brief-artifact-type

title: Add the Brief artifact type to product-cli's catalog

@motivated_by brief:pipeline-worker-slice-1
@addresses_decision adr:brief-as-typed-artifact

Adds Brief as a first-class artifact type in product-cli with the shape proposed
in this working session: title, premise, goal, success_criteria, plus the edges
`decomposes_into → Feature[]`, `excludes → Feature[]`, `acknowledges →
Acknowledgement[]`, `references → Artifact[]`.

Bootstrap Feature — required for this Brief to be authorable into the graph
through `product author`. Until it ships, this file remains a free-form markdown
working document.

scope:
  - SHACL shape for Brief
  - product-cli schema migration to add the type
  - `product author` recognizes `## Brief <id>` sections
  - Brief-aware queries surface for "show me all Features decomposed from
    Brief-X" and "show me everything Brief-X excluded"

out_of_scope:
  - Nested Briefs (the granularity question — defer until 2-3 Briefs have been
    authored and patterns emerge)
  - Brief versioning / supersession (defer until needed)

---

## Feature feature:wire-layer

title: SSE consumer and HTTP poster for the dispatch/completion protocol

@motivated_by brief:pipeline-worker-slice-1
@addresses_decision adr:wire-protocol-sse-post
@addresses_decision adr:wire-payload-nquads

Implements the only layer that knows the network exists. Maintains a long-lived
SSE connection to pipeline-cli's dispatch endpoint, advertises the worker
process's capability tags, resumes with `Last-Event-ID` on reconnect. Posts
completion events back via HTTP with retry on transient failures. Issues atomic
claim requests on incoming dispatches to handle multi-worker capability tag
contention. Caches model catalog responses per worker process.

Surfaces dispatches to the Session layer via an async iterator.

---

## Feature feature:session-layer

title: One dispatch → one completion lifecycle with in-memory pyoxigraph store

@motivated_by brief:pipeline-worker-slice-1
@addresses_decision adr:pyoxigraph-in-memory
@addresses_decision adr:session-as-prov-activity

The unit of measurement on the worker side. Mirrors pipeline-cli's session
record on the other side. Owns an in-memory pyoxigraph store that holds this
session's bundle sub-graph for the session's lifetime. Accumulates telemetry
across all provider calls and side-channel emissions. On clean exit, serializes
the artifact triples + side-channel triples + telemetry into a completion
payload. On exception, emits whatever side-channel triples were captured and
posts a `blocked` or `escalated` completion.

The Session IS a `prov:Activity`. Mechanical provenance annotations on the
artifact (`prov:wasGeneratedBy`, `prov:wasAttributedTo`, `prov:used`) are
populated by pipeline-cli's GraphWriter from the session record, not by the
worker.

---

## Feature feature:bundle-layer

title: Curated query helpers over the in-memory bundle sub-graph

@motivated_by brief:pipeline-worker-slice-1
@addresses_decision adr:bundle-accessors-codegen

Wraps the session's in-memory pyoxigraph store with role-specific typed
accessors generated from the role's bundle SHACL shape. Workers call
`bundle.focal()`, `bundle.linked_adrs()`, `bundle.applicable_test_criteria()` —
they never write SPARQL by hand. The accessors are deterministic and idempotent:
same store, same query, same return.

Raw store access remains available via `bundle.raw_store` for diagnostic and
exceptional cases, flagged in telemetry to surface gaps in the curated surface.

---

## Feature feature:artifact-layer

title: Typed artifact builders with SHACL validation at commit

@motivated_by brief:pipeline-worker-slice-1
@addresses_decision adr:artifact-builders-codegen

Typed builders mapped to each role's output SHACL shape. Workers call
`artifact.set_title(...)`, `artifact.link_to(uri, predicate=...)`,
`artifact.commit()`. The builder enforces required-field population and runs
pyshacl conformance against the shape before passing triples back to the
session. Same shapes pipeline-cli will revalidate against on receive — defensive
check on the worker side, authoritative check at the boundary.

Raw triple emission available via `artifact.emit_triple(s, p, o)` for shape-
conformant cases the typed surface doesn't cover; flagged in telemetry.

---

## Feature feature:provider-abstraction

title: LiteLLM client with capability-tag dispatch and structured output

@motivated_by brief:pipeline-worker-slice-1
@addresses_decision adr:litellm-as-provider-substrate
@addresses_decision adr:capability-tag-binding
@addresses_decision adr:structured-output-via-instructor
@addresses_decision adr:configurable-provider-endpoint

The dispatch event names a capability tag and a model binding. The Provider
layer maps the capability tag to a LiteLLM model group (configured in the
LiteLLM proxy deployment per brief:worker-distribution-slice-1's
feature:litellm-proxy-deployment) and calls LiteLLM's OpenAI-compatible
endpoint with that model group, the worker's bundle-derived prompt, the
structured-output schema (instructor on top of LiteLLM), and metadata that
propagates our session ID through.

LiteLLM handles: provider translation, virtual key auth, fallbacks, retries,
provider-specific parameter passthrough (Anthropic tool use, OpenAI
response_format, etc.) via `extra_body`. The SDK does not write per-provider
adapters; per-provider behavior is configured in the LiteLLM proxy, not in
worker code.

Per-call surface (illustrative):

```python
response = await provider.complete(
    capability_tag="frontier-reasoning",   # → LiteLLM model group
    messages=[...],
    output_schema=ADRSchema,               # via instructor
    metadata={"ddd_session_id": session.id},
)
```

The Provider implementation reads LiteLLM's endpoint URL from `LITELLM_BASE_URL`
and its virtual key from `LITELLM_API_KEY`, both injected via the
`pipeline-cli workers run` env config. Defaults to `http://localhost:4000`
for local-host LiteLLM (slice 1); production deployments override.

Telemetry capture has two layers:
- Synchronous: the call's tokens, latency, model, retry count, provider chosen
  (when LiteLLM routes / falls back) are captured locally and attached to the
  session's telemetry block.
- Asynchronous: LiteLLM's logging callback POSTs to pipeline-cli's
  `/llm-call-telemetry` endpoint with the same fields plus cost (LiteLLM
  computes from token counts and provider pricing). pipeline-cli reconciles
  against the worker-reported telemetry; mismatches are a fitness function
  on the proxy.

Authoritative source for the session record: the worker's own telemetry
report (in the completion event). LiteLLM's callback is the verification
feed. Where they diverge, the worker report wins for provenance; LiteLLM's
cost figure is taken as authoritative for spend tracking specifically (it
sees the actual provider invoice line).

For workers using tools mid-session (deferred to slice 3 with the implementer
role), the full tool-call lineage will be captured in one session's
telemetry; LiteLLM supports multi-turn tool-use exchanges with the same
session-metadata propagation.

---

## Feature feature:side-channel

title: Emergent judgments and feedback emission

@motivated_by brief:pipeline-worker-slice-1

Implements the two APIs from impl doc §6:

- `session.record_emergent_judgment(decision, rationale)` for in-authority
  judgments. Surfaces in the produced artifact's metadata; reviewed by the
  paired interpretation session.
- `session.emit_feedback(class, severity, evidence, blocking=...)` for
  out-of-authority issues. Emits a Feedback artifact conforming to the
  feedback schema. Blocking feedback causes the session to exit with
  `outcome=blocked`; non-blocking flows alongside `outcome=completed`.

Both emit triples into the session's emission set, packaged into the
completion event alongside the main artifact.

---

## Feature feature:driver-interface

title: Driver abstraction for production and replay

@motivated_by brief:pipeline-worker-slice-1

Defines the `Driver` interface that both EventDriver (production) and
ReplayDriver (offline replay, slice 2) implement. Workers consume sessions via
`async for session in driver:` and never know which driver invoked them. This
is what operationalizes "per-role queries are the unit of evolution" (impl doc
§4) — the same worker code runs against historical bundles offline as runs
against live dispatches.

Slice 1 ships the interface plus EventDriver. ReplayDriver implementation is
slice 2.

---

## Feature feature:event-driver

title: Production driver implementation

@motivated_by brief:pipeline-worker-slice-1

The concrete `Driver` for production: subscribes to pipeline-cli's SSE
endpoint, issues atomic claims, hands sessions to worker code, posts
completions. Composed from the wire layer.

---

## Feature feature:shape-codegen

title: Build-time codegen of typed Bundle and Artifact surfaces from SHACL

@motivated_by brief:pipeline-worker-slice-1
@addresses_decision adr:codegen-build-time

Reads SHACL shapes from pipeline-cli/schemas/ and generates the typed Bundle
accessors and Artifact builders. Output is checked in. CI on both repos runs
codegen and fails on drift. This is what enforces the shared-shape principle:
what pipeline-cli's harness packed is what the SDK exposes, with no semantic
drift between sides.

---

## Acknowledgement ack:security-deferred

@motivated_by brief:pipeline-worker-slice-1

Slice 1 holds a static bearer token on the worker process for both the SSE
connection and completion POSTs. No rotation, no scoping beyond capability
tags, no multi-tenancy. Acceptable because slice 1 is single-tenant and runs
locally. Token rotation, fine-grained scoping, and multi-tenant authentication
become real concerns when more than one worker process operates against the
same pipeline-cli, deferred to slice 4+.

---

## Acknowledgement ack:full-provenance-discipline-separate

@motivated_by brief:pipeline-worker-slice-1
@references brief:dual-provenance-discipline

The universal dual-provenance discipline (mechanical provenance auto-attached
on every artifact, motivational provenance required per type via SHACL) is a
framework-level schema change affecting product-cli's entire catalog. It
belongs in its own sibling Brief, not this one. This Brief depends on it for
the Brief artifact type's SHACL shape to be definable in conformance with the
discipline.

If brief:dual-provenance-discipline is not authored before this Brief's
feature:brief-artifact-type lands, that Feature implements provisional
provenance rules that must be reconciled when the discipline is formalized.

---

## ADR adr:brief-as-typed-artifact

@decides_for feature:brief-artifact-type

Bounds documents have been free-form markdown wrappers around the structured
product-cli graph (impl doc §11). This works only because no slice's bounds
have crossed role boundaries yet. The moment a Brief needs to be referenced
by other artifacts (a Feature querying "what scope decision motivated me," a
later slice's Brief checking the prior Brief's `excludes` list), the bounds
document needs a typed shape and an addressable ID.

Decision: introduce Brief as a typed artifact in product-cli's catalog with
the shape developed in this working session. Future bounds documents are
authored as Brief artifacts, not free-form markdown.

Alternatives considered:
  - Extend Feature with a parent edge for scope grouping. Rejected: conflates
    "what gets shipped" with "what frames the shipping."
  - Keep bounds documents free-form, link by URL. Rejected: no schema means
    no validation, no queryability, no discipline.

---

## ADR adr:wire-protocol-sse-post

@decides_for feature:wire-layer

Dispatches are a broadcast event stream with replay semantics; completions are
validated RPC submissions with synchronous response semantics. Different
protocol shapes, asymmetric by design.

Decision: SSE for dispatches (harness → worker), HTTP POST for completions
(worker → harness).

Alternatives considered:
  - WebSocket for both directions. Rejected: gains a single connection but
    re-invents replay semantics, request/response correlation, and HTTP
    backpressure inside a custom frame protocol. Doesn't earn the complexity.
  - NATS bidirectional pub/sub. Deferred: the right escalation if push
    latency or message rate ever exceed what SSE+POST can support. State
    stays in Oxigraph; NATS would only carry wake-up signals.

---

## ADR adr:wire-payload-nquads

@decides_for feature:wire-layer

The wire carries RDF, not arbitrary JSON. Two serialization candidates:
N-Quads for fidelity (preserves named-graph membership, no information loss
at edges), JSON-LD for ergonomics (workers see structured objects).

Decision: N-Quads on the wire. SDK converts to a Python-friendly view
internally. Fidelity at the boundary, ergonomics one layer above.

Revisit if the conversion overhead becomes measurable or if a non-Python
worker SDK ever needs to be written and JSON-LD's tooling proves stronger
than N-Quads' there.

---

## ADR adr:capability-tag-binding

@decides_for feature:provider-abstraction

Role-to-model bindings reference capability tags ("frontier-reasoning",
"code-specialized", "fast-cheap"), not model names. The catalog maps tags to
concrete provider+model at dispatch time.

Decision: the SDK's provider layer consumes capability tags from dispatch
events and resolves via the catalog. Workers never see model names. New
model qualifies → catalog updates → no SDK or worker change required.

Without this, the model catalog (impl doc §9) is decorative — model names
would be hardcoded in workers and rebinding would require code changes.

---

## ADR adr:codegen-build-time

@decides_for feature:shape-codegen

Two options for SHACL → typed-surface generation: build-time (generated module
checked in) or runtime (read shapes at import, generate dynamically).

Decision: build-time. IDE and type-checker friendly; predictable startup; SDK
release boundary matches shape-version boundary, which is auditable.

Trade: SDK must release when shapes change. Acceptable at current cadence;
revisit if shape churn becomes the bottleneck.

---

## ADR adr:pyoxigraph-in-memory

@decides_for feature:session-layer

The bundle is an in-memory RDF sub-graph for the duration of one session. Two
candidates: pyoxigraph (Python binding to Oxigraph, same engine pipeline-cli
uses) or rdflib (pure-Python, mature, slower).

Decision: pyoxigraph. Same SPARQL engine, same SHACL implementation, same
serializations on both sides of the wire. A bundle query written for
pipeline-cli's assembly can be unit-tested by the SDK in-process. No semantic
drift across the boundary.

---

## ADR adr:session-as-prov-activity

@decides_for feature:session-layer

The Session is not just an SDK abstraction — it IS a `prov:Activity` in the
PROV-O graph. The session record in pipeline-cli is its persistent form; the
in-process Session object is its live form. Mechanical provenance annotations
on produced artifacts (`prov:wasGeneratedBy`, `prov:used`, `prov:wasAssociatedWith`)
trace through Session as the central node.

Decision: model the SDK's Session as a direct materialization of the PROV-O
Activity for its dispatch. This unifies session telemetry, audit trail, and
provenance graph traversal into one structural concept.

---

## ADR adr:artifact-builders-codegen

@decides_for feature:artifact-layer

Typed builders for artifact emission could be hand-written per role or
generated from SHACL shapes.

Decision: codegen from shapes, same pipeline as bundle accessors. One source
of truth (the shape), two generated surfaces (read-side accessors, write-side
builders). Hand-written escape hatch (`emit_triple`) for shape-conformant
cases the typed surface misses.

---

## ADR adr:structured-output-via-instructor

@decides_for feature:provider-abstraction

LLM responses need to map to artifact builder calls reliably. Options:
provider-native structured output (Anthropic tool use, OpenAI response_format),
instructor/Pydantic for uniform structured output across providers, or BAML.

Decision: instructor + Pydantic as the uniform surface, using provider-native
structured output under the hood where available. Provides one schema-coercion
behavior across providers; concrete provider implementations remain thin.

Revisit if instructor proves limiting for any specific provider or if BAML's
guarantees become structurally needed.

---

## ADR adr:configurable-provider-endpoint

@decides_for feature:provider-abstraction
@references brief:worker-distribution-slice-1
@references adr:litellm-as-llm-proxy-slice-1   (in worker-distribution Brief)

The Provider layer reads LiteLLM's endpoint URL from `LITELLM_BASE_URL` and
its virtual key from `LITELLM_API_KEY`. Defaults to `http://localhost:4000`
for the slice-1 local-host LiteLLM deployment; production deployments
override to point at the actual LiteLLM service.

This is what makes the LiteLLM deployment swappable. The SDK doesn't know
or care whether LiteLLM is on localhost, in a sidecar container, on a shared
host, or behind a load balancer. One env var moves it.

Trade: workers depend on LiteLLM being reachable at the configured URL.
Acceptable because LiteLLM is part of the slice-1 deployment per
worker-distribution Brief's feature:litellm-proxy-deployment, and the
`pipeline-cli workers run` subcommand ensures the env vars are set
correctly before starting a worker container.

---

## ADR adr:litellm-as-provider-substrate

@decides_for feature:provider-abstraction
@references brief:worker-distribution-slice-1

Originally the Provider layer was to be a multi-provider abstraction the SDK
owns — separate Anthropic, OpenAI, Scaleway implementations conforming to a
common interface, plus capability-tag-to-(provider, model) resolution via a
catalog the SDK consumes.

Evaluation surfaced that this is exactly what LiteLLM already does, with
substantially more features (virtual keys, rate limits, fallbacks, spend
tracking, logging callbacks, response caching). Building it ourselves would
duplicate work at a layer that's already commoditized in the LLM tooling
ecosystem.

Decision: use LiteLLM as the provider substrate. The SDK's Provider layer
is a thin client of LiteLLM's OpenAI-compatible API. Per-provider behavior
is configured in LiteLLM's deployment, not in worker code. Capability tags
map to LiteLLM model groups.

Why this isn't framework-lock-in (the way LangChain/AutoGen would be):
LiteLLM is a wire-level translator/proxy, not a composition framework. It
doesn't impose how work is structured or how agents compose. The analogy:
rejecting LangChain is like rejecting Rails (a framework that defines app
shape). Accepting LiteLLM is like accepting `requests` (a library that
handles a specific layer well). No conflict with the "graph is yours,
artifacts are the interface" stance from impl doc §2.

Competing source-of-truth concern (LiteLLM has its own session model,
virtual keys with budgets, spend tracking): addressed by declaring our
session record authoritative for everything DDD cares about (provenance,
bundle hash, role, motivational origin, downstream consequences); LiteLLM's
records are operational state for proxy concerns (rate limits, fallback
decisions, key budgets) and a verification feed for cost reconciliation.
Where they overlap, our store wins. The one exception: LiteLLM's cost
figure is taken as authoritative for spend tracking specifically, because
LiteLLM sees the actual provider invoice line.

OpenAI-shaped API at the worker layer is acceptable — de facto standard,
provider-specific features still accessible via `extra_body`.

Alternatives considered:

- **Per-provider SDK wrappers** (the original plan). More code we own, more
  maintenance, no observability or routing wins. Rejected.
- **OpenRouter or similar SaaS proxy.** Same architectural fit as LiteLLM
  but hosted; introduces a third-party runtime dependency on the critical
  path. Self-hosting LiteLLM keeps the dependency at the library level.
- **Litellm-as-library only (no proxy server).** Use LiteLLM's Python SDK
  in-process per worker instead of running the proxy server. Loses
  centralized key management, centralized logging, and the fact that
  multiple worker processes can share one running LiteLLM. Rejected;
  proxy is the right deployment shape.

Slice 1 ships LiteLLM with one model group (Anthropic via the provider's
API). Additional providers (OpenAI, Scaleway, Bedrock, etc.) are added by
editing LiteLLM's config, not by SDK changes.

---

## Open questions

These are inline narrative for slice 1 — promote to OpenQuestion artifacts in
their own right when the discipline supports it (likely with
brief:dual-provenance-discipline).

1. **Brief ID convention.** Used `brief:<slug>` here. Product-cli may have
   conventions for ID generation (UUIDs? IRIs under a base namespace?). Adopt
   whatever exists.

2. **Predicate names.** Used `@decomposes_into`, `@motivated_by`,
   `@addresses_decision`, `@references`. These map onto SHACL predicate URIs
   one level down. The actual URIs come from the schema work in
   brief:dual-provenance-discipline.

3. **`product author` parse rules.** Option-2 format works in chat. Whether
   `product author` already parses something close, or whether this slice's
   feature:brief-artifact-type also defines the parser, is open. Likely the
   parser change is part of the Feature.

4. **Whether brief:dual-provenance-discipline is authored before, alongside,
   or after this Brief.** If before: this Brief's Brief-shape SHACL conforms
   to the dual-provenance discipline from the start. If alongside: the two
   develop together with reconciliation passes. If after: ack:full-provenance-
   discipline-separate calls out the reconciliation debt.

5. **LiteLLM session-metadata propagation depth.** LiteLLM's `metadata` field
   propagates to logging callbacks but does it propagate to upstream provider
   logs (Anthropic's request_id, OpenAI's request IDs)? If yes, end-to-end
   correlation from our session_id to provider invoice line is trivial. If
   no, we need a separate correlation key. Validate during implementation.

6. **LiteLLM virtual key issuance.** Slice 1 has one virtual key shared across
   all workers (or one per worker process). When does the discipline shift to
   per-WorkerImage virtual keys? Likely with feature:litellm-proxy-deployment's
   slice-2 hardening; tracked there rather than here.
