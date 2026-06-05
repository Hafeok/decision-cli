---
id: ADR-083
title: Tech detail binds at exactly one level — archetype-invariant, instance-bound, or feature-bound
status: accepted
features:
- FT-148
- FT-149
supersedes: []
superseded-by: []
domains:
- api
- data-model
scope: cross-cutting
content-hash: sha256:73142c81960b0565712f112571e2e7321c9d363e8527cbb50de3487b617169ed
---

**Status:** Proposed

## Context

Under [ADR-082](ADR-082), the catalog gains three places a technical decision can live: the **ApplicationContract** (archetype-invariant), the **InfrastructureContract** (instance-bound, frozen at Discovery per customer), and a **TaskType parameter** (feature-bound, varying per dispatch). The two contracts plus the per-task parameter map is the cleanest way to express that one archetype can serve many customers — but only if every tech detail finds its right home. Getting the binding level wrong produces one of two failure modes the spec warns about:

- **A rigid catalog that cannot serve two customers.** If "Azure SQL" gets baked into the ApplicationContract rather than the InfrastructureContract, a customer wanting Postgres Flexible Server cannot use the archetype — they would need a different archetype despite sharing the application stack. The application TaskTypes would refuse to dispatch against a Postgres-backed instance because their contract said SQL Server.
- **A vague catalog whose audits do not hold.** If "C# / .NET 9" gets pushed down to a TaskType parameter, every cell prompt has to negotiate the language at dispatch time, the archetype audits cannot mechanically assert layering (because the language is unknown until late), and the contracts have lost their teeth.

The litmus tests from `briefs/system-archetype-spec-v2.md §2` give the binding rule directly: if changing the detail would change every cell prompt in the archetype, it is archetype-invariant; if it varies between customers but does not change the application cell prompts, it is instance-bound; if it varies per task at dispatch, it is feature-bound. The witnessed examples from the spec:

| Detail | Should bind at | Reasoning |
|---|---|---|
| C# / .NET 9 | Application contract | Changing it changes every application cell prompt; a customer wanting Go is asking for a different archetype, not a parameter |
| Clean Architecture's dependency rule | Application contract | Layering is the rule audits assert against; if it varies per instance, no archetype-wide audit can hold |
| Vertical-slice feature organisation | Application contract | The on-disk layout assembly puts artifacts into; cells generate against it |
| "SQL domain model" + EF Core conventions | Application contract | The persistence *model*, not the engine; the application derives from the model, the engine is downstream |
| Azure SQL *vs* Postgres Flexible | Infrastructure contract | Both satisfy "SQL domain model"; the choice is per-customer; the application does not change |
| Container Apps *vs* App Service *vs* AKS | Infrastructure contract | Different deployment targets; application code agnostic to which |
| Entra External ID *vs* B2C | Infrastructure contract | Identity provider; per-customer regulatory / commercial choice |
| Key Vault | Infrastructure contract | Concrete secrets backing; application reads via the contract's "secrets" slot |
| "Does this feature need a new table?" | TaskType parameter | Per-feature decision at dispatch; varies within an archetype |
| "Does this feature need a new Service Bus topic?" | TaskType parameter | Same |

These look obvious enumerated like this. They are not obvious when an archetype is being authored under deadline pressure and a customer is asking about variance. The rule needs to be ADR-grade — written down, checkable, citable — not vibes.

The decision-cli self-implementation archetype ([FT-160](FT-160)) is going to exercise this rule first. Some of the bindings there are not obvious:

- The role catalog's per-role tool surfaces (ADR-070, ADR-071, FT-121) — archetype-invariant? Yes: every instance of the decision-cli archetype runs the same role catalog because the catalog is part of the archetype's safety property.
- The LiteLLM proxy's concrete endpoint (`http://localhost:4000` for local dev, a deployed URL for production) — instance-bound. The application contract names a "LiteLLM proxy" abstract slot; the instance pins the URL.
- The capability tag a worker requests (`anthropic/claude-opus-4-7` *vs* `scaleway/qwen3-coder-480b`) — TaskType parameter? Or instance-bound? Here the rule's edge case bites: it is TaskType parameter (each cell asks for the capability it needs), but the *resolver* the parameter feeds into is instance-bound (an instance with no Anthropic key cannot serve `anthropic/*` capabilities — it has to map them to a substitute via the InfrastructureContract's capability map).

Writing this ADR resolves these edge cases by codifying the rule + the litmus tests + a tie-breaker.

## Decision

**Every technical detail referenced by a TaskType, cell, or audit binds at exactly one of three levels: archetype-invariant (ApplicationContract), instance-bound (InfrastructureContract), or feature-bound (TaskType parameter). The binding level is determined by the litmus tests below and is mechanically checked.**

### 1. The three levels and their litmus tests

Apply in order; the first match wins:

1. **Archetype-invariant** (binds in ApplicationContract). The detail satisfies *both* of:
   - Changing it would change every cell prompt in the archetype (the "prompt-pervasive" test).
   - It does not vary between customers of the archetype (the "customer-invariant" test).
   
   Examples: language/runtime, layering rule, slice organisation, persistence *model*, endpoint convention, cross-cutting conventions (auth model, validation pipeline, error handling, logging).

2. **Instance-bound** (binds in InfrastructureContract). The detail satisfies *all* of:
   - It varies between customers of the archetype (so it cannot be archetype-invariant).
   - It does NOT change the application cell prompts (so the contract format is "we have an X slot"; the application cells derive from the slot, not from the concrete choice).
   - It must be set once per customer at Discovery and frozen thereafter (so it cannot be per-dispatch).
   
   Examples: compute target (Container Apps vs App Service), data engine (Azure SQL vs Postgres satisfying the application's "SQL domain model"), identity provider, secrets backing, messaging substrate, observability stack, the LiteLLM proxy's concrete URL.

3. **Feature-bound** (binds as a TaskType parameter). The detail satisfies *all* of:
   - It varies per task at dispatch (so it cannot be customer-frozen).
   - The set of legal values is constrained by the contracts (the contracts say what the slots are; the parameter says which slot value this task uses).
   - The audit can mechanically check the parameter is valid against the contracts.
   
   Examples: "this feature wants a new table" (legal because the application's persistence model says "SQL domain model"; the parameter picks `new-table` vs `extend-existing-table`), "this feature wants a new Service Bus topic" (legal only if the InfrastructureContract includes Service Bus; the parameter picks `new-topic` vs `existing-topic`).

### 2. The tie-breaker — the "prompt-pervasive" test

When the litmus tests above are ambiguous, the tie-breaker is the prompt-pervasive test from the spec: would changing this detail force a rewrite of every application cell prompt in the archetype? If yes, it is archetype-invariant. If no, it is one of the other two.

The witnessed edge case from the decision-cli archetype: the capability resolver shape. Application cells say `model_binding_capability_iri: "openai/code-small"`; the resolver maps that to a concrete model at dispatch via the InfrastructureContract's capability map. The capability-tag *vocabulary* (`code-small`, `code-specialist`, `deep-reasoning`) is archetype-invariant because every cell prompt references it. The *mapping* from tag → endpoint + model_id is instance-bound because each customer's InfrastructureContract pins their available providers. The cell does not pick the model; the resolver does. So:

- Capability tag vocabulary → ApplicationContract.
- Resolver / capability map → InfrastructureContract.
- Per-cell tag choice → TaskType parameter (each cell declares what it asks the resolver for).

This is the rule's payoff: applied consistently, the edge case has a single answer.

### 3. Mechanical check — `tech-detail-binding-level` audit

A new platform TC backed by `scripts/checks/tech-detail-binding-level.sh` walks the catalog and asserts:

1. **Every detail referenced in an ApplicationContract is referenced from at least one application cell prompt.** A detail in the contract that no cell uses is suspect — either it should not be archetype-invariant (move down) or it represents tribal knowledge the cells silently rely on (write the cell).
2. **No detail in an ApplicationContract varies across the archetype's instances.** Iterate over `instances/{id}/infrastructure.contract.md`; if any instance's contract overrides a detail also declared in the ApplicationContract, that detail is mis-bound (move to InfrastructureContract).
3. **No detail in an InfrastructureContract appears in an application cell prompt as a concrete value.** Application cell prompts may reference contract *slots* (`{infra-contract:data-engine}.persistence`) but never the concrete instance value (`Azure SQL`). Direct concrete references are mis-bound.
4. **No TaskType parameter has a value set outside the constraints declared by the contracts.** A parameter saying "new Service Bus topic" against an InfrastructureContract without Service Bus is a binding violation.

The check exits 0 on pass, 1 on violation with a diagnostic naming the detail, the level it is bound at, and the level the rule says it should bind at. Runs through `product verify --platform`.

### 4. Application changes drive archetype boundary checks, not parameter additions

A common failure mode the rule prevents: a customer asks for variance, and the responder reaches for "I will add a TaskType parameter for it" because the parameter layer is the cheapest to extend. The rule refuses this when:

- The variance forces a change to an application cell prompt → it is not a parameter, it is an archetype-invariant property the customer disagrees with. They want a different archetype.
- The variance is between customers but does not change cell prompts → it is an InfrastructureContract slot, not a TaskType parameter. Set it once per customer and freeze.

If the rule says "this is a different archetype," that is a real diagnostic the catalog wants surfaced. A customer wanting Go instead of C# is asking for an *Internal Tool (Go / Cloud Run)* archetype, not a Self-Service Portal (.NET / Azure) with a "language" parameter. The catalog grows by minting archetypes, not by widening one archetype's parameter surface until it serves everyone.

## Rejected alternatives

### Two levels — collapse instance-bound into feature-bound

Drop the InfrastructureContract; let every per-customer detail flow through TaskType parameters. Rejected — re-introduces the variance ADR-082 §Rejected §1 wanted out. Per-customer details have to be set once and frozen; a per-dispatch parameter cannot enforce freeze; the customer ends up with conflicting choices across features in the same archetype.

### Two levels — collapse instance-bound into archetype-invariant

Bake the infrastructure choices into the archetype; each customer becomes a separate archetype. Rejected — destroys the cross-customer reuse the archetype layer exists to provide. Per ADR-082 §1, one archetype serves many customers precisely because the application layer holds invariant; collapsing instance-bound into archetype-invariant requires N archetypes for N customers and the catalog economic model breaks.

### Single level — every detail is a parameter

Maximum flexibility, no contracts at all. Rejected by ADR-082 directly. Without contracts there is no upstream cell to fix the variance the catalog economic model requires, no place for audits to assert conformance, and the broad worker re-derives every decision per dispatch. The whole catalog returns to flat TaskTypes with implicit contracts.

### Litmus tests in docs, not in an ADR

Write the rule into the spec; do not give it an ADR. Rejected per [ADR-014](ADR-014) — fitness functions live as ADRs + TCs, not as docs. The mechanical check (`tech-detail-binding-level`) needs an ADR home; the rule it enforces is exactly the kind of cross-cutting invariant ADR-014 governs.

### No mechanical check — rely on human review at archetype-author time

Trust the catalog authors to apply the rule. Rejected — the rule's failure modes are silent. An archetype-invariant detail mis-bound as instance-bound shows up as drift across instances years later, not at authoring time. A TaskType parameter referencing a contract slot that doesn't exist shows up as a runtime failure at the first customer who tries the parameter. The mechanical check catches all four classes at PR time.

## Consequences

### Positive

- **Archetype boundary diagnostics surface early.** "This is actually a different archetype" becomes a recognisable diagnostic, raised by the rule, not by a customer escalation six months in.
- **Catalog growth is principled.** New archetypes are minted because the rule said so; existing archetypes get parameter additions only when the rule allows.
- **Edge cases have answers.** The capability-resolver case has a single binding answer applied consistently; future edge cases (e.g., "where does the OCI registry URL bind?") get walked through the same litmus tests.
- **Audit teeth strengthen the contracts.** The mechanical check turns the rule into enforcement; a violation blocks merge through `product verify --platform`.

### Negative / accepted trade-offs

- **The check needs careful authoring.** Detecting "every detail in the ApplicationContract is referenced by at least one cell" requires parsing both contract markdown and cell prompts. False positives (a contract detail genuinely unused but harmless) are possible; the check's first version is conservative (`scripts/checks/tech-detail-binding-level.sh` lands as a feature-level slice within FT-148).
- **Author cost on contract drafting.** Each contract item must explicitly state its binding level and pass the litmus test in its conventions file. Cost paid at archetype-authoring time; benefit is the mechanical check has something to read.
- **Pushes some catalog growth toward "mint a new archetype" rather than "extend the existing one".** This is the correct outcome — fewer Frankenstein archetypes with parameter surfaces covering disjoint use cases — but it shifts effort.
- **The check has limited reach on instance-only details.** When a customer adds an InfrastructureContract instance with a detail no audit references, the check cannot prove the detail is correctly placed; it can only prove the contract slots are filled and the value is valid against the slot. Detail placement is human review at Discovery time.

### Relationship to prior decisions

- **[ADR-082](ADR-082)** introduces the three layers this rule binds details to. ADR-082 is the structural decision; ADR-083 is the routing rule between the layers.
- **[ADR-014](ADR-014)** establishes cross-cutting fitness functions as ADRs + TCs; this ADR is one such fitness function.
- **[ADR-033](ADR-033)** introduced capability-based model routing. The capability *vocabulary* binding (archetype-invariant) plus capability *map* binding (instance-bound) under this ADR is the explicit codification of what ADR-033 implicitly assumed.
- **[FT-067](FT-067) / [FT-068](FT-068)** routed verify-graph-author through the resolver. The "resolver shape is invariant; concrete bindings are per instance" pattern those features assumed is the rule this ADR is generalising.

## Status

Proposed. Promotes to accepted once FT-148 (ApplicationContract) and FT-149 (InfrastructureContract) ship the contract types and `scripts/checks/tech-detail-binding-level.sh` runs green against the decision-cli self-implementation archetype (FT-160) with no false positives on the witnessed bindings.
