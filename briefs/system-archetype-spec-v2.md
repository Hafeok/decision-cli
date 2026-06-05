# System Archetype Specification — v2

**The top-level structure of the Context& catalog.**

This spec defines what a *System Archetype* is, what it contains, and the rules that make
its task types dispatchable. It sits one layer above `forge/task-types/` and gives the
catalog a frame: task types are not flat — they belong to an archetype, and derive their
coherence from the contracts that archetype pins.

> **What changed in v2.** A system instance is pinned by **two parallel contracts**, not
> one: an **Application Architecture Contract** (archetype-invariant — language, layering,
> slice organization, persistence model) and an **Infrastructure Contract** (instance-bound
> — the concrete Azure resources, chosen once per customer and frozen). Application task
> types and infrastructure (Bicep) task types derive from their respective contracts. A new
> **seam audit** checks that the two agree. This split is what lets one archetype serve
> many customers: the application layer stays customer-invariant while the infrastructure
> layer flexes.

> **The load-bearing idea.** A task type is only dispatchable on a cheap model if there is a
> fixed frame for its cells to agree *within*. The contracts are that frame — the furthest-
> upstream cells in the DDD funnel. Every task type is downstream translation of a well-
> specified problem into the idiom the contracts already chose. Set the contracts once, and
> the variance that would otherwise make features un-typeable collapses.

---

## 1. The layers

```
System Archetype                       ← the unit you sell and instantiate
├─ Application Architecture Contract    ← archetype-invariant; upstream cell (app)
├─ Infrastructure Contract              ← instance-bound; upstream cell (infra), frozen at Discovery
├─ Task-Type Set
│    ├─ Application task types          ← derive_from the application contract
│    └─ Infrastructure task types       ← derive_from the infrastructure contract
└─ Audits
     ├─ Per-type coherence audits       ← cells of one task type agree
     ├─ Archetype audits                ← conformance to a contract
     └─ Seam audit                      ← app contract ⟷ infra contract agree (load-bearing)
```

| Layer | What it is | Varies per customer? | Cost profile |
|---|---|---|---|
| **Application Architecture Contract** | Language, runtime, layering rule, feature organization, persistence *model* | **No** — invariant for the archetype | Set once when the archetype is authored |
| **Infrastructure Contract** | Concrete cloud resources: compute, data engine, identity provider, secrets, messaging | **Yes** — chosen per instance, then frozen | Set once per instance at Discovery |
| **Application task types** | Generators for archetypal features (pages, CRUD, profile, auth rules) | No | Cheap, repeated, small/code models |
| **Infrastructure task types** | Generators for Bicep/IaC units (databases, secrets, topics, identities) | No (the generators); their *inputs* come from the instance's infra contract | Cheap, repeated; **stateful/ordered** (see §7) |
| **Audits** | Coherence checks made definable *because* the contracts are fixed | No | Cheap, automated |

The archetype is the unit of cross-customer reuse. Two customers in unrelated business
domains share a *Self-Service Portal* archetype even though they share nothing at the domain
layer. **Archetypes overlap far more than domains do** — and within an archetype, the
**application contract overlaps across *all* customers** while only the infrastructure
contract flexes.

---

## 2. The three kinds of tech detail (where each binds)

Getting this wrong produces either a rigid catalog that can't serve two customers, or a
vague one whose audits don't hold. Every tech detail belongs to exactly one of these:

| Kind | Examples | Binds at | Lives in |
|---|---|---|---|
| **Archetype-invariant** | C# / .NET 9, Clean Architecture, vertical slices, "SQL domain model", EF Core conventions | When the archetype is authored | Application Architecture Contract (§4) |
| **Instance-bound** | Azure SQL *vs.* Postgres Flexible, Container Apps *vs.* App Service, Entra External ID, Key Vault, Service Bus | Once per customer at Discovery, then **frozen** | Infrastructure Contract (§5) |
| **Feature-bound** | "new table or project existing?", "needs a new Service Bus topic?" | Per task, at dispatch | Task-type parameters (§6) |

> **Litmus test for the first two.** If changing the detail would change every cell prompt
> in the archetype, it is archetype-invariant (e.g. the language). If it varies between
> customers but does not change the application cell prompts, it is instance-bound (e.g.
> Azure SQL vs. Postgres — the app derives from "SQL domain model", not from the concrete
> engine). A customer who wants Go instead of C# is asking for a **different archetype**,
> not a parameter of this one.

---

## 3. Directory layout

```
forge/archetypes/{archetype-id}/
  archetype.yaml                  # the archetype manifest (§ schema below)
  application/
    contract.md                   # APPLICATION architecture contract — invariant (§4)
    conventions/
      slice.md                    # vertical-slice layout
      clean-architecture.md       # the dependency rule
      persistence.md              # SQL domain-model + EF Core conventions
      frontend-contract.md        # how the frontend consumes endpoints
      cross-cutting.md            # auth, validation, error handling, logging
  infrastructure/
    contract.template.md          # INFRASTRUCTURE contract — the slots an instance fills (§5)
    conventions/
      naming.md                   # resource naming / tagging
      networking.md               # vnet/private endpoints stance
      identity.md                 # managed identity / role-assignment conventions
  task-types/
    application/{task-type-id}/... # derive_from application contract
    infrastructure/{task-type-id}/...# derive_from infrastructure contract; carry ordering (§7)
  audits/
    {audit-id}.md                 # archetype + seam audits (§8)
  instances/
    {instance-id}/
      infrastructure.contract.md  # the FROZEN infra choices for this customer
      record.md                   # evidence: repo, commit, coverage
  EVIDENCE.md                     # occurrences, variance, coverage (§9)
```

A task type under an archetype follows the **same internal structure** from the Pattern
Extraction Playbook (`task-type.yaml`, `cells/`, `coherence-audit.md`, `applicability.md`,
`examples/`, `EVIDENCE.md`). The additions are the `archetype` / `conforms_to` fields (§6)
and, for infrastructure types, the ordering fields (§7).

---

## 4. The Application Architecture Contract (`application/contract.md`)

The upstream cell for application work — invariant across every instance of the archetype.
Once fixed, every application task type is specifiable. It must state, checkably:

1. **Language & runtime** — e.g. C# / .NET 9. This is baked into every application cell
   prompt; it is *not* metadata on the side.
2. **Layering rule** — e.g. Clean Architecture's dependency rule (dependencies point inward;
   domain has no outward references). Stated as a *checkable* rule.
3. **Feature organization** — e.g. vertical slices: one folder per feature with its
   command/query, handler, endpoint, tests.
4. **Domain-model conventions** — the persistence *model* (SQL domain model, entity → table
   mapping, EF Core conventions, migration discipline). Note: the *model*, not the concrete
   engine — the engine is instance-bound (§5).
5. **Endpoint/contract convention** — how a backend capability is exposed and how the
   frontend consumes it (the shape that makes endpoint == contract == frontend-call == test
   checkable).
6. **Cross-cutting conventions** — auth model, validation pipeline, error handling, logging:
   the rules every task type inherits without re-deciding.

Each item links to a `application/conventions/` file precise enough that an **audit** can
mechanically check conformance. A convention that cannot be checked cannot be an audit, and
task types depending on it are not safely dispatchable.

---

## 5. The Infrastructure Contract (`infrastructure/contract.template.md` → instance)

The upstream cell for infrastructure work — **a parallel contract**, set once per customer
at Discovery and then **frozen** for the life of the instance. The template declares the
slots; each instance fills them in `instances/{id}/infrastructure.contract.md`.

Typical slots for an Azure archetype:

1. **Compute** — Azure Container Apps *or* App Service *or* AKS.
2. **Data engine** — Azure SQL *or* Postgres Flexible Server (must satisfy the application
   contract's "SQL domain model" — that is the binding rule between the two contracts).
3. **Identity provider** — Entra External ID *or* B2C *or* external IdP.
4. **Secrets** — Key Vault (and how the app reads it).
5. **Messaging** — Service Bus / Event Grid, if the archetype needs it.
6. **Observability** — App Insights / Log Analytics workspace.

Rules:

- Every instance choice must **satisfy the application contract**. "SQL domain model"
  (§4.4) is satisfied by either Azure SQL or Postgres; it is *not* satisfied by Cosmos.
  Record the satisfaction explicitly.
- The infrastructure contract is the home for the **Bicep catalog**. Infrastructure task
  types derive their inputs from this contract (the concrete resource set), exactly as
  application task types derive from the application contract.
- Once frozen, instance infra choices change only through an explicit re-contracting step,
  never silently at dispatch time.

> This is DDD's **IaC cell** made first-class: it runs parallel to the application contract,
> owns its own domain decisions (region, sizing, tagging, networking), and feeds a distinct
> family of task types.

---

## 6. Task types within an archetype

A task type adds, to its `task-type.yaml`:

```yaml
archetype: self-service-portal
family: application            # application | infrastructure
conforms_to:                   # named conventions it must obey
  - clean-architecture
  - slice
  - persistence
cells:
  - id: query-handler
    artifact: "C# query handler in the feature slice"
    funnel_position: downstream
    model_binding: code-specialized
    derived_from: ["app-contract:slice", "app-contract:clean-architecture"]
```

`derived_from: ["app-contract:{c}"]` or `["infra-contract:{c}"]` means the cell reads a
fixed contract convention rather than re-deriving it. This is what moves the hard reasoning
upstream and lets the cell run on a small model — it is not deciding *how* to layer or
*which* resources exist; the contracts did that. It is only filling in the feature.

**Applicability still required.** Each task type states when it applies and when it does not
(the classifier needs it). Within an archetype this is easier: "`add-list-page` applies when
the feature is a read-only tabular view of a domain entity; it does NOT apply when the view
requires domain computation beyond filter/sort/page — that routes to the broad worker."

---

## 7. Infrastructure task types have state and ordering

Application task types are pure generation: dispatch → audit → done. **Infrastructure task
types are not** — they have real-world state, side effects, and ordering. You cannot add a
Key Vault secret before the Key Vault exists; granting a role requires the resource and the
identity to be present. Infrastructure `task-type.yaml` therefore carries two extra fields:

```yaml
family: infrastructure
provisioning:
  depends_on: ["azure-keyvault-module"]   # ordering against other infra types
  idempotency: declarative                 # declarative (Bicep what-if safe to re-apply)
                                            # | imperative (guard against double-apply)
  side_effects: true                       # touches real cloud state
```

Rules:

- `depends_on` is honored at dispatch: an infra type is not dispatched until its
  dependencies are satisfied for the instance.
- Prefer `idempotency: declarative` (Bicep `what-if` / deployment stacks) so re-application
  is safe and drift is detectable. Imperative steps must declare a guard.
- Infra dispatch is gated on the instance's **frozen** infrastructure contract — never on
  ad-hoc choices made mid-dispatch.

---

## 8. Audits — three scopes

| Scope | Lives in | Checks |
|---|---|---|
| **Per-type coherence** | with the task type | The cells of *one* task type agree (handler conforms to its own contract). |
| **Archetype audit** | `audits/` | Conformance to **a contract** across any dispatched work. |
| **Seam audit** | `audits/` | The **application and infrastructure outputs agree**. (New in v2; load-bearing.) |

Examples:

| Audit | Scope | What it checks | Enabled by |
|---|---|---|---|
| `slice-conforms-to-clean-architecture` | archetype (app) | No outward dependency from domain; layering rule holds | §4.2 |
| `endpoint-contract-test-alignment` | archetype (app) | Route/shape == frontend call == test path | §4.5 |
| `migration-matches-domain-model` | archetype (app) | Every domain-model change has a migration; no drift | §4.4 |
| `bicep-conforms-to-naming` | archetype (infra) | Resources follow naming/tagging conventions | §5 / naming.md |
| **`app-config-matches-iac-outputs`** | **seam** | App's expected connection strings / endpoints == IaC outputs | §4.6 ⟷ §5 |
| **`app-identity-matches-iac-roles`** | **seam** | The managed identity the app assumes == the roles IaC grants | §4.6 ⟷ §5.3 |
| **`app-resource-expectations-met`** | **seam** | Every resource the app reads (KV secret, SB topic) is provisioned | §4 ⟷ §5 |

> **Why the seam audit is the most important detail in v2.** Application and infrastructure
> are generated by different cells under different contracts, so *nothing makes them agree
> unless an audit forces it.* A misconfigured managed identity or a connection-string
> mismatch is exactly the "subtle integration bug a single broad-agent context would have
> caught for free" that DDD warns about — and it is precisely the failure that surfaces
> *after* handover and destroys Business Continuity margin. Naming and enforcing the seam
> audit protects the handover promise.

**Load-bearing requirement (from DDD).** Every audit — and the seam audit especially — must
be **at least as strong as what a single broad agent gets for free** from shared context. If
it is weaker, the decomposition is worse than the monolith for that concern. Any audit that
cannot meet this bar is marked `candidate / audit weak` and its dependent task types are
flagged *not safely dispatchable* until strengthened.

---

## 9. Evidence and coverage honesty (`EVIDENCE.md`)

Per archetype, record:

- **Instances**: real systems on this archetype (repo + commit), and their variance.
- **Coverage**: which archetypal features the task-type set ships, and which known archetypal
  features are *not yet* covered (the gaps in the 80%).
- **Layer split**: application-layer vs. infrastructure-layer task types vs. domain-layer
  leakage that should not be in the archetype set at all.
- **Contract split health**: confirm the application contract held invariant across instances
  while only the infrastructure contract varied. If an "instance difference" forced an
  *application* contract change, that is a signal the archetype boundary is wrong.
- **Seam regression results**: outcome of regenerating known-good instances via dispatch and
  running the seam + archetype audits (per Extraction Playbook §3). This validates that the
  audits — the load-bearing ones — actually hold.

---

## 10. How an archetype is used (lifecycle)

```
Discovery     Classify the customer's system → match to an archetype.
              Application contract is already fixed (archetype-invariant).
              Set & FREEZE the Infrastructure Contract for this instance (the Azure choices).
              Measure: % of features that are archetype-layer (the 80%) vs. domain-layer
              (the 20% for the broad worker) → scope + value input.

Build         Dispatch application + infrastructure task types to ship the 80% cheaply.
              Infra types respect provisioning order (§7); broad worker handles domain 20%,
              minting domain task types. Archetype + SEAM audits run after every dispatch.

Continuity    Operate the instance. Stable because conformance to BOTH contracts and the
              seam between them is machine-checked, not hoped-for. Low-constraint → cheap.
```

The archetype bridges the strategy (custom solutions at platform economics) and the catalog
(dispatchable task types). The two contracts are why the 80% is real and why one archetype
serves many customers; the task-type set is the 80% itself; the audits — the seam audit
above all — are why Business Continuity can safely inherit the result.

---

## 11. Hard rules

1. **Two contracts, not one.** Application Architecture Contract (invariant) and
   Infrastructure Contract (instance-bound, frozen at Discovery). Never collapse them.
2. **Tech detail binds at exactly one level** (§2). Archetype-invariant → application
   contract; instance-bound → infrastructure contract; feature-bound → task-type parameter.
3. **The infrastructure contract must satisfy the application contract.** A data-engine
   choice must satisfy "SQL domain model"; record the satisfaction explicitly.
4. **A seam audit is mandatory and load-bearing.** App ⟷ IaC must be machine-checked. No
   archetype ships without it.
5. **Coverage is per-archetype, never per-customer-system.** The domain 20% is real and
   separate; never market the archetype 80% as a whole-system 80%.
6. **No archetype without checkable contracts.** A vibe is not a contract.
7. **Every audit must meet the monolith bar** (§8). Weaker → task types flagged not safely
   dispatchable.
8. **Infrastructure task types declare ordering & idempotency** (§7). App types do not need
   to; infra types must.
9. **Archetypes are the unit of cross-customer reuse.** Domain task types that don't
   generalize stay out of the archetype set (logged, not promoted).
10. **One archetype at a time.** Prove one archetype's contracts, task types, and seam audit
    before spreading. Self-Service Portal (.NET / Azure) first.
11. **Status promotion (`candidate → standard`) is always a gated human decision.**
