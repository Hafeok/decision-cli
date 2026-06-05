---
id: ADR-082
title: Archetype as the layer above TaskType — two parallel contracts and three-scope audits
status: accepted
features:
- FT-147
- FT-148
- FT-149
- FT-150
- FT-151
- FT-152
- FT-154
- FT-155
- FT-157
- FT-160
supersedes: []
superseded-by: []
domains:
- api
- data-model
- observability
scope: cross-cutting
content-hash: sha256:e5d910026944515338c095af25c0afdb766097d4037499024db416cf1bad3373
---

**Status:** Proposed

## Context

[ADR-080](ADR-080) split decision-cli's self-implementation pipeline into TaskTypes + Cells: a recognised feature dispatches a cluster of typed cells with a per-task coherence audit; the broad code-writer is the unknown-task escape hatch. [FT-139](FT-139) shipped the substrate and the `add-judge-worker` prototype; [FT-140..FT-144](FT-144) added five more TaskTypes covering the witnessed roadmap. Six TaskTypes, one coherence audit per type, a static registry compiled into the binary, and an explicit broad-worker fallback.

The pattern works at the scale we have it now. What is missing is a frame *above* the TaskTypes that explains why two of them can share a contract and a third cannot, why ordering between types matters when one provisions a resource a sibling reads, and where the audit lives that catches the most damaging integration drift — the one between code that *runs in* an environment and the IaC that *creates* that environment. The witnessed instances of this gap inside decision-cli itself: code-writer assumes `LITELLM_BASE_URL` at runtime, the LiteLLM-proxy worker-distribution slice ([FT-096](FT-096)) provisions it, and nothing in the TaskType layer makes those two facts agree — the agreement is held by a human reading both. The same shape will scale badly the moment the catalog admits a second deployment target.

The briefs (`briefs/system-archetype-spec-v2.md`, `briefs/feature-authoring-brief.md`, `briefs/pattern-extraction-playbook-v2.md`) name the missing layer **System Archetype** and specify its shape with three load-bearing claims:

1. A system instance is pinned by **two parallel contracts** — an Application Architecture Contract (archetype-invariant: language, layering, slice organisation, persistence *model*, endpoint convention, cross-cutting) and an Infrastructure Contract (instance-bound: concrete cloud resources, frozen at Discovery per customer). Application TaskTypes derive from the first; infrastructure TaskTypes derive from the second. One archetype serves many customers because the application layer is customer-invariant while only the infrastructure layer flexes.
2. TaskTypes split into **two families** — `application` (pure generation; dispatch → audit → done) and `infrastructure` (stateful, ordered, side-effecting; declares `depends_on`, `idempotency`, `side_effects`). The current decision-cli TaskTypes are all application-family by default; the split becomes load-bearing the moment an archetype owns IaC.
3. Audits live at **three scopes** — per-type coherence (the existing FT-139 audit), archetype conformance (the cells of any dispatched work conform to a contract), and **seam audit** (application output ⟷ infrastructure output agree). The seam audit is new in v2 and is the load-bearing detail of the whole spec: it catches the misconfigured managed identity, the connection-string mismatch, the role IaC granted but the app doesn't assume — the exact class of failure that surfaces *after* handover and destroys Business Continuity margin.

The TaskTypes built under ADR-080 are the downstream half of this picture; the contracts and the seam audit are the upstream half that has been implicit so far. This ADR makes the upstream half first-class.

## Decision

**Adopt the System Archetype as the catalog layer above TaskType. An archetype owns two parallel contracts, a TaskType set split by family, and audits at three scopes. The seam audit is mandatory.**

Concretely:

### 1. Archetype is a typed, graph-resident artifact

A new artifact type `dec:Archetype` becomes part of the catalog ontology. Each archetype has:

- An `id`, `title`, and `status: candidate | standard` (`candidate` only ever minted automatically; promotion is gated per [ADR-085](ADR-085)).
- A link to its **ApplicationContract** (one — invariant for the archetype).
- A link to its **InfrastructureContract template** (one — the slot specification).
- A set of **Instance bindings**, each linking to a frozen InfrastructureContract instance per customer.
- A set of **TaskTypes**, split by `family: application | infrastructure`.
- A set of **archetype audits** and **seam audits**.

The substrate is shipped as a typed ontology artifact (Rust struct + SHACL shape + parser + emitter), not as a feature_spec body convention. The `FT-TT-<name>` body-as-substrate convention from FT-139 was the right v1 minimum; FT-150 promotes TaskType + Cell to first-class artifact types as part of the same slice that introduces Archetype, closing the bootstrap loop ADR-080 deferred.

### 2. Two parallel contracts, never one

The contracts live in distinct artifact types because their lifecycles differ:

- **ApplicationContract** is archetype-invariant. It states *checkably* the language/runtime, the layering rule, the feature-organisation shape, the persistence model, the endpoint/contract convention, and the cross-cutting conventions. Each item links to a conventions file precise enough that an audit can mechanically verify conformance. A convention that cannot be checked cannot be an audit, and TaskTypes depending on it are not safely dispatchable.
- **InfrastructureContract** has a template (the archetype declares the slots) and instances (each customer fills the slots once and freezes). The template owns the Bicep catalog input slots; the instance pins concrete choices (Azure SQL *vs* Postgres, Container Apps *vs* App Service, Entra External ID *vs* B2C).

The contracts must not be collapsed. The litmus test from the spec: if changing a detail would change every cell prompt, it belongs in the ApplicationContract; if it varies between customers but does not change application cell prompts, it belongs in the InfrastructureContract; if it varies per task at dispatch, it is a TaskType parameter. [ADR-083](ADR-083) makes this binding rule cross-cutting.

### 3. TaskTypes split by family

The existing `TaskType` artifact (post-FT-150) gains a `family: application | infrastructure` field, `conforms_to: [convention_id]` declaring which contract conventions it obeys, and `derived_from-contract` references on each cell that pull from `app-contract:<convention>` or `infra-contract:<convention>` rather than re-deriving the convention. Cells reading a contract convention do not re-decide it — the contract is the upstream cell.

Infrastructure TaskTypes additionally declare `provisioning.depends_on`, `idempotency`, and `side_effects` ([FT-151](FT-151)). They cannot run before their dependencies are satisfied. They must be idempotent (declarative Bicep, `what-if` safe) or carry an explicit guard against double-apply. Application TaskTypes do not need any of this — they remain pure generation.

### 4. Three audit scopes; seam audit is mandatory

The existing per-type coherence audit (FT-139) stays. Two new scopes appear:

- **Archetype conformance audits** (`slice-conforms-to-clean-architecture`, `endpoint-contract-test-alignment`, `bicep-conforms-to-naming`) — conformance to a contract across any dispatched work.
- **Seam audits** (`app-config-matches-iac-outputs`, `app-identity-matches-iac-roles`, `app-resource-expectations-met`) — application output ⟷ infrastructure output.

Per [ADR-084](ADR-084), no archetype ships without a non-empty seam-audit set that meets the *monolith bar*: each audit must be at least as strong as a single broad agent's free coherence over a shared context. An audit weaker than that is marked `candidate / audit weak` and its dependent TaskTypes are flagged *not safely dispatchable*.

### 5. The dispatch loop is six-step and walkable

Per `briefs/feature-authoring-brief.md`, the dispatch agent's loop is fixed: **CLASSIFY → PLAN → DISPATCH → AUDIT → ASSEMBLE → REPORT**. The classifier is allowed to refuse — a unit that does not cleanly match a TaskType routes to the escape hatch ([FT-154](FT-154)) rather than forcing a near-miss dispatch. Infrastructure-first ordering is honoured: an application unit reading a resource cannot dispatch until the infrastructure TaskType that provisions it has dispatched and audited green. Assembly places artifacts in their conventional locations per the ApplicationContract's feature-organisation rule.

[FT-157](FT-157) lands the planner that implements this loop.

### 6. Coverage is per-archetype, never per-customer-system

The catalog's coverage claim is over the archetype's archetypal-layer features — the ~80% the type set ships. The domain-layer ~20% is real and separate; it routes through the broad worker per ADR-080 and is logged as a TaskType candidate per the pattern-extraction playbook but is never marketed as part of the archetype's coverage. EVIDENCE.md per archetype distinguishes application-layer, infrastructure-layer, and domain-layer counts; a customer system's whole-system coverage is the archetype's archetype-layer coverage *plus* whatever domain-layer work the broad worker shipped, never the archetype's number alone.

### 7. One archetype at a time

The decision-cli self-implementation pipeline becomes the first archetype ([FT-160](FT-160)). It backfills the existing TaskTypes (FT-139..FT-144) into an explicit Archetype with an ApplicationContract (Rust workspace + vertical-slice + SDP boundary at `oxi-events`/`core`/`features` + product-cli for engineering artifacts) and an InfrastructureContract (the LiteLLM proxy + worker-image OCI registry + Scaleway/Anthropic capability bindings). The first seam audit lands here: every worker's capability binding endpoint must match an InfrastructureContract instance output, every role catalog seed must reference a Capability resolvable through the resolver. If the first archetype's audits do not have teeth — if they pass on the live repo while the witnessed drifts (the implicit `LITELLM_BASE_URL` agreement above) go unflagged — the decomposition fails its own load-bearing test and the layer needs revision before a second archetype lands.

## Rejected alternatives

### Keep TaskTypes flat; do not introduce an Archetype layer

Status quo post-FT-144. Rejected — the catalog has no place to express the fact that `add-judge-worker` and `add-author-worker` share an application contract while `add-cli-subcommand` arguably shares only some of it, no place to express that an infrastructure TaskType has to run before an application TaskType that reads its output, and no place for the seam audit to live. The witnessed drift between code-writer's `LITELLM_BASE_URL` assumption and the LiteLLM-proxy provisioning slice is exactly the failure mode the seam audit catches; flat TaskTypes have nowhere to host that check.

### One contract instead of two

Merge ApplicationContract and InfrastructureContract into a single `SystemContract`. Cleaner schema, fewer artifact types. Rejected — collapses the spec's load-bearing distinction. The contracts have different lifecycles (application is archetype-invariant; infrastructure is instance-frozen), different audiences (application TaskTypes vs infrastructure TaskTypes), and different evolution patterns (application changes are archetype-wide; infrastructure changes are per-customer). A single contract either forces customer churn on archetype-wide decisions or admits per-instance fields that re-introduce drift between application and infrastructure. The spec is explicit (`§11.1`): "Two contracts, not one. Never collapse them."

### Make the seam audit advisory, not mandatory

Ship Archetypes with a seam audit slot but allow it to be empty. Rejected by `§11.4` and reinforced by [ADR-084](ADR-084) — the seam audit is precisely the audit DDD's own analysis identifies as load-bearing for safe handover. Shipping an archetype without one ships the pattern without its safety property; the type system says "this is fine" while exactly the bug class the layer exists to catch goes uncaught. The catalog's promise to Business Continuity rests on the seam audit being non-optional.

### Skip status gating; promote archetypes automatically when audits pass

Audit-driven auto-promotion: when an archetype's seam audit has demonstrably caught a real drift, flip `candidate → standard`. Rejected per [ADR-085](ADR-085) — automatic promotion lets a broad-worker mining run mint a `standard` archetype, which is the opposite of "broad worker emits only candidates" from the pattern-extraction playbook. Promotion is a human review of evidence (instance count, variance, audit-catch history, coverage honesty); coupling it to audit signal alone reproduces the misclassification risk the candidate/standard split exists to manage.

### Build the Archetype layer abstractly first; defer the first archetype

Land the Archetype + ApplicationContract + InfrastructureContract types, the audit pipeline, the classifier and dispatcher, with no archetype actually instantiated. Wait for the second use case before committing. Rejected — the spec's `§11.10` ("one archetype at a time") and the playbook's regression-test requirement (`playbook §7`) both demand a known-good instance to validate the substrate against. Building the layer without an archetype is exactly the "type system without a witness" failure mode ADR-080 already warned against. The decision-cli self-implementation pipeline is a real archetype with real instances (FT-126/127/128.. shipped through `add-judge-worker`); using it as the first archetype both validates the layer and proves the seam audit has teeth on a system we can read end-to-end.

### Backfill the existing TaskTypes silently into an implicit decision-cli archetype

When the Archetype substrate lands, automatically attach FT-139..FT-144 to a default archetype without an explicit migration. Rejected — the migration is the archetype's evidence. The act of writing the ApplicationContract for decision-cli (Rust + vertical-slice + SDP at the named boundaries + product-cli as the engineering substrate) is what tests whether the contract is *checkable*. If it is not — if "vertical slice" is a vibe rather than a SHACL-or-script-enforceable rule — the contract is incomplete and the TaskTypes claiming to conform to it are not safely dispatchable. Silent backfill hides that diagnostic. The explicit FT-160 migration surfaces it.

## Consequences

### Positive

- **The catalog has a frame.** TaskTypes belong to an archetype; their cells derive from contract conventions; the audits fan out at three scopes from the contracts. The "why do these two TaskTypes go together?" question is answered by the contracts, not by a comment.
- **The seam audit catches the worst drift class.** App ⟷ IaC agreement becomes a machine-checked invariant. The witnessed implicit agreement between code-writer's `LITELLM_BASE_URL` assumption and the LiteLLM-proxy slice gets a home and a check. New seams (a worker assuming a Key Vault secret, a CLI assuming an OCI registry URL) carry seam-audit obligations from day one rather than accumulating as untyped tribal knowledge.
- **One archetype serves many instances.** Application TaskTypes are customer-invariant; only the infrastructure contract flexes. The economic model that ADR-080 implicitly assumed becomes structurally true: a second customer of the decision-cli archetype reuses every application TaskType and only re-freezes its InfrastructureContract.
- **Infrastructure ordering is data, not narrative.** `depends_on` between infrastructure TaskTypes is declared, validated at registration time, and honoured by the dispatcher. "You can't add a Key Vault secret before the Key Vault exists" stops being a code-review concern and becomes a planner refusal.
- **Maturation curve is visible per archetype.** EVIDENCE.md tracks application-layer vs infrastructure-layer vs domain-layer counts per archetype. The 80% claim is auditable; the broad worker's share is measurable; the domain-layer signal informs which vertical to pursue next.

### Negative / accepted trade-offs

- **Significant new substrate to ship.** Archetype, ApplicationContract, InfrastructureContract template + instance, the family/conforms_to/derived_from-contract extensions on TaskType, SeamAudit, ArchetypeAudit, the three-scope dispatch pipeline, the escape-hatch routing — at least eight features in the slice (FT-147..FT-160). Mitigated by ordering: substrate first (147..150), audits + escape hatch next (151..154), workers + planner (155..157), CLI + first archetype (158..160). Each landing is a small, verifiable slice.
- **Contract authoring is the new hard problem.** Writing an ApplicationContract precisely enough that audits can mechanically check conformance is harder than writing a TaskType prompt. The first archetype's contract is the test of whether the framework's contract format is workable; the playbook's regression-test step (`§7`) is the safety net.
- **Status promotion adds a new human-in-the-loop step.** Per ADR-085, no archetype auto-promotes to `standard`. This is a feature, not a bug — it prevents broad-worker mining runs from minting `standard` archetypes — but it does mean the catalog's `standard` set grows only as fast as humans review evidence.
- **The classifier becomes a soft-signal layer.** The v1 classifier matches `task_type:` declarations on feature_specs (FT-139's path); the archetype-aware classifier from FT-155 adds applicability-clause matching against the archetype's TaskType set. False negatives route to the escape hatch (per ADR-080, that is correct); false positives dispatch wrong clusters, which the per-type + archetype + seam audits must catch. The audits are the safety property; the classifier's job is to fail safe.
- **Bootstrap pressure on FT-160.** The first archetype is also the test of whether decision-cli's own self-implementation can be described as one. If the ApplicationContract cannot capture the SDP-at-`oxi-events`/`core`/`features` rule checkably, the contract format is incomplete; if the InfrastructureContract cannot capture the LiteLLM-proxy + OCI-registry + Scaleway-endpoint shape coherently, the template slots are incomplete. Either signal would block the slice. The risk is real and the diagnostic is real — that is the point.

### Relationship to prior decisions

- **[ADR-080](ADR-080)** introduced TaskType + Cell and made the broad worker the unknown-task path. This ADR adds the layer above and keeps the broad-worker escape hatch unchanged (now at the archetype-classifier level rather than just the TaskType level).
- **[FT-139](FT-139)** shipped the TaskType + Cell substrate as feature_spec bodies under the `FT-TT-<name>` convention. FT-150 promotes them to first-class artifact types as part of this slice — the bootstrap deferral ADR-080 §Rejected §3 named.
- **[ADR-081](ADR-081)** required CLI enumerate/lookup totality. The `dec archetype list/show` pair from FT-158 lands under this totality rule from day one — `cli_pairing.rs` registers the new pair.
- **[FT-066](FT-066) / [FT-067](FT-067) / [FT-096](FT-096)** ship the witnessed seam: code-writer expects `LITELLM_BASE_URL`; the LiteLLM-proxy slice provisions it; nothing in the type system today asserts they agree. The first archetype's seam audit ([FT-160](FT-160)) makes that assertion explicit and gives it teeth.
- **[ADR-014](ADR-014)** established cross-cutting fitness functions as ADRs + TCs. The seam audit per [ADR-084](ADR-084) is a per-archetype fitness function: it lives inside the archetype directory rather than under `.product/`, but the principle (audits are graph-resident, not config) is the same.

## Status

Proposed. Promotes to accepted once the substrate (FT-147..FT-154) ships, the dispatch pipeline (FT-155..FT-157) lands, and the first archetype (FT-160) regenerates a known-good instance with the seam audit catching at least one drift the type system would otherwise have missed — the same load-bearing-prototype standard ADR-080 set for the first TaskType.
