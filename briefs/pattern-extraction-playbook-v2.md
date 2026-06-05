# Pattern Extraction → Archetype Catalog Playbook — v2

**Audience: a broad-authority coding agent (the explorer-and-typifier) running against
Context& repos.**

You are operating as the *broad worker* in a Decision Driven Design (DDD) system. Your job
in this run is **not** to ship features. It is to mine the existing repos for an
**archetype** — its two contracts and the task types that derive from them — and emit them
as catalog entries a cheap, typed generator can dispatch forever after. Your most valuable
output is the discovery of a task type, not the code.

This playbook is aligned to the **System Archetype Specification v2** (two parallel
contracts + seam audit). Read both before starting. Work **one archetype at a time**, scoped
to repos that are instances of the *same* system archetype (e.g. all the self-service
portals). Be conservative: a wrong task type is worse than a missing one, because it
dispatches confidently-wrong clusters downstream.

---

## 0. Definitions you must hold

- **System Archetype** — a recurring *kind of system* (Self-Service Portal, Internal Admin
  Tool, Approval Workflow). The unit of cross-customer reuse. You are extracting ONE.
- **Application Architecture Contract** — archetype-invariant decisions: language, runtime,
  layering rule, feature organization, persistence *model*. Every application cell prompt is
  written against it. If a detail would change every prompt, it belongs here.
- **Infrastructure Contract** — instance-bound decisions: the concrete cloud resources
  (compute, data engine, identity, secrets, messaging). Varies per customer, frozen per
  instance. Home of the Bicep catalog.
- **Pattern** — a structure repeating across instances (a Bicep module shape, a .NET
  slice cluster, an auth wiring).
- **Cell** — one prompt → one artifact. A cell exists where a sub-artifact **crosses a
  boundary to inform a different downstream decision** (the crossing test). Naming inside one
  function crosses nothing — not a cell.
- **Task type** — a named, parameterized generator owning a cluster of cells, with schema,
  applicability, `derived_from` ordering, model bindings, coherence audit. Belongs to a
  `family`: `application` or `infrastructure`. This is the catalog entry.
- **Coherence audit** — the named check that a task type's cells agree.
- **Seam audit** — the check that **application output and infrastructure output agree**
  (app config ⟷ IaC outputs; assumed identity ⟷ granted roles; resources read ⟷ resources
  provisioned). Load-bearing. This is where the worst handover bugs live.

---

## 1. Output contract (the archetype directory)

Emit into `forge/archetypes/{archetype-id}/`, matching spec v2 §3:

```
forge/archetypes/{archetype-id}/
  archetype.yaml                       # manifest (§2 below)
  application/
    contract.md                        # APPLICATION contract — invariant (§3)
    conventions/{name}.md              # checkable conventions (slice, clean-arch, persistence, …)
  infrastructure/
    contract.template.md               # INFRASTRUCTURE contract — the slots an instance fills (§4)
    conventions/{name}.md              # naming, networking, identity
  task-types/
    application/{task-type-id}/...     # family: application
    infrastructure/{task-type-id}/...  # family: infrastructure (carry ordering — §5)
  audits/
    {audit-id}.md                      # archetype audits AND seam audits (§6)
  instances/
    {instance-id}/record.md            # each real system you derived from (repo, commit)
  EVIDENCE.md                          # §7
```

Each task type keeps the standard internal structure: `task-type.yaml`, `cells/{id}.md`,
`coherence-audit.md`, `applicability.md`, `examples/sources.md`, `EVIDENCE.md`.

---

## 2. `archetype.yaml`

```yaml
id: self-service-portal
name: "Self-Service Portal (.NET / Azure)"
status: candidate                 # you ONLY ever emit candidate
application_contract: application/contract.md
infrastructure_contract: infrastructure/contract.template.md
coverage:
  archetype_layer_estimate: 0.0   # fill from EVIDENCE; fraction of THIS ARCHETYPE's features covered
  note: "Coverage of archetypal features only — never a customer's whole system."
task_types:
  application: []                 # ids you minted
  infrastructure: []
audits:
  archetype: []                   # e.g. slice-conforms-to-clean-architecture
  seam: []                        # e.g. app-config-matches-iac-outputs  (MUST be non-empty)
maturity_evidence:
  instances: 0
  repos: []
  variance: low|medium|high
  application_contract_held_invariant: true|false   # see §7
```

---

## 3. Infer the Application Architecture Contract

From the repos in scope, derive the **invariant** application decisions and write
`application/contract.md` (spec v2 §4). State each *checkably* and link a `conventions/`
file. Required items:

1. **Language & runtime** (e.g. C# / .NET 9) — observed consistently across instances.
2. **Layering rule** (e.g. Clean Architecture dependency rule) — stated as a checkable
   constraint.
3. **Feature organization** (e.g. vertical slices) — the on-disk shape.
4. **Persistence model** (e.g. SQL domain model + EF Core conventions) — the *model*, not
   the engine. The engine is instance-bound.
5. **Endpoint/contract convention** — how capabilities are exposed and consumed (the shape
   that makes endpoint == contract == frontend-call == test checkable).
6. **Cross-cutting** — auth, validation, error handling, logging.

**Invariance check:** an item belongs here only if it holds across *all* in-scope instances.
If instances disagree on a supposed invariant, either (a) it is actually instance-bound (move
to §4), or (b) you are looking at two different archetypes (stop, split, report).

---

## 4. Infer the Infrastructure Contract

Write `infrastructure/contract.template.md` as the **slots** instances fill (spec v2 §5),
plus one filled `instances/{id}/...` per real system you found. Typical slots: compute, data
engine, identity provider, secrets, messaging, observability.

- For each slot, record the concrete choices observed across instances and whether they
  **vary** (instance-bound — expected) or are **constant** (candidate to promote into the
  application contract if it would change cell prompts).
- **Satisfaction rule:** record how each data-engine choice satisfies the application
  contract's persistence model ("Azure SQL satisfies 'SQL domain model'"). Flag any instance
  whose infra does NOT satisfy the app contract — that is a real defect or a mis-scoped
  archetype.
- This is where the **Bicep catalog** lands: infrastructure task types derive their inputs
  from these slots.

---

## 5. Extract task types (both families)

For each candidate pattern cluster (apply the crossing test for cell boundaries; ≥3
occurrences across ≥2 instances = strong candidate; 1–2 = record but mark high variance):

### 5a. Common to both families
- Decompose one representative instance into cells. Contract/interface → cell; tests → cell;
  IaC → cell; intra-artifact naming → not a cell.
- Write `applicability.md`: **when it applies / when it does NOT / what each parameter
  switches on / what decision it encodes and why.** No decision rationale → do not mint;
  log as rejected. This is the misclassification guard.
- Write `cells/{id}.md` from the real instances. Cells `derive_from` contract conventions
  (`app-contract:slice`, `infra-contract:data-engine`) rather than re-deriving them.
- Write `coherence-audit.md`: what must agree within the type, checkably, at least as strong
  as a single agent's free coherence.
- Set `family:` and `conforms_to:` in `task-type.yaml`.

### 5b. Application family specifics
- Cells `derive_from` `app-contract:*`. Hard problem-domain reasoning concentrates upstream
  (the contract); downstream cells translate into the fixed idiom on small/code models. A
  downstream cell that seems to need a frontier model = the contract under-specified; log it.

### 5c. Infrastructure family specifics — ordering & state (spec v2 §7)
Infra types are NOT pure generation. Add to `task-type.yaml`:
```yaml
family: infrastructure
provisioning:
  depends_on: ["azure-keyvault-module"]   # ordering vs other infra types
  idempotency: declarative                 # prefer Bicep what-if-safe; imperative needs a guard
  side_effects: true
```
Derive `depends_on` from the real provisioning order in the repos (you cannot add a secret
before its vault). Prefer declarative/idempotent Bicep.

---

## 6. Author the audits — including the seam audit (load-bearing)

Under `audits/`, produce:

- **Archetype audits** — conformance to a contract, definable *because* the contract is
  fixed. E.g. `slice-conforms-to-clean-architecture`, `endpoint-contract-test-alignment`,
  `migration-matches-domain-model`, `bicep-conforms-to-naming`.
- **Seam audits (mandatory, non-empty)** — application output ⟷ infrastructure output:
  - `app-config-matches-iac-outputs` — connection strings / endpoints the app expects ==
    what IaC emits.
  - `app-identity-matches-iac-roles` — the managed identity the app assumes == roles IaC
    grants.
  - `app-resource-expectations-met` — every resource the app reads (KV secret, SB topic) is
    provisioned by infra.

For each audit state what it checks and how a machine checks it. **Every audit must meet the
monolith bar** — at least as strong as a single broad agent's free coherence. If it cannot,
mark it `candidate / audit weak` and flag dependent task types *not safely dispatchable*.

---

## 7. Regression test (validate before claiming the archetype works)

Use the repos as a ready-made test suite (spec v2 §9, DDD's own advice to build the audit
first). For the archetype:

1. Pick a **known-good instance** (record commit).
2. Derive the input specs a requester would have provided for a sample of its features.
3. **Regenerate** those features by dispatching the relevant task types in `derived_from` /
   `depends_on` order — application AND infrastructure.
4. Run the **archetype audits and the seam audits** against the regenerated output.
5. Compare against the known-good original (behavioral match, not byte-identical).

Record in `EVIDENCE.md`:
- Did the per-type, archetype, and **seam** audits pass?
- Did regenerated output match the original's behavior?
- Did any cell need a bigger model than its binding (→ upstream under-specification)?
- **Did the audits — the seam audit especially — catch anything a single context would have
  caught for free?** If weaker than the monolith, say so; that audit is not ready.
- **`application_contract_held_invariant`**: did the app contract stay fixed across instances
  while only infra varied? If an "instance difference" forced an *application* contract
  change, the archetype boundary is wrong — report it.

---

## 8. Hand-back report (`EXTRACTION-REPORT.md`)

- **Archetype identified**: id, the two contracts, instances found, variance.
- **Census**: candidate clusters, occurrences, repos.
- **Minted**: application task types and infrastructure task types, with maturity evidence.
- **Layer split**: application-layer vs. infrastructure-layer vs. **domain-layer leakage**
  (logic that recurs because *our customers' business* recurs — NOT archetypal; keep it out
  of the set, log it, note whether it generalizes).
- **Contract-split health**: did the application contract hold invariant? Any forced app
  changes?
- **Seam audit status**: are the mandatory seam audits defined and do they meet the monolith
  bar? (If not, the archetype is not shippable.)
- **Rejected**: patterns found but not minted, with reason.
- **Coverage estimate**: archetypal features covered vs. known gaps.
- **Niche signal**: where domain-layer density is highest (informs which vertical to pursue).

---

## 9. Hard rules (do not violate)

1. **Emit only `status: candidate`.** Promotion to `standard` is a gated human decision.
2. **Two contracts.** Infer both; never collapse them. Invariant → application; instance-bound
   → infrastructure.
3. **No task type without an applicability decision** (§5a). Code without rationale → rejected.
4. **No dispatchable type without a definable coherence audit; no archetype without a seam
   audit** that meets the monolith bar.
5. **Infrastructure types declare ordering & idempotency** (§5c). Application types need not.
6. **Coarse, parameterized types over narrow proliferation.** When unsure, merge.
7. **Conservative on low evidence.** 1–2 occurrences → record, don't invest.
8. **Never modify the source repos.** Read + extract only. Output goes under
   `forge/archetypes/`. Repos are evidence and test suite, not worktree.
9. **Distinguish application / infrastructure / domain-layer in every report entry.**
10. **One archetype per run.** Stay scoped to instances of the same system kind.
