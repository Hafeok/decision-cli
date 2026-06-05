# Feature Authoring Brief

**Audience: a dispatch agent building a feature inside an already-instantiated System
Archetype.**

You are shipping ONE feature into an existing system that was built on a System Archetype.
The archetype's two contracts are already set and frozen for this instance. Your job is to
translate a feature request into working, coherent code by **dispatching the archetype's
task types** — not by free-form building. The hard architectural decisions were made once,
upstream, when the contracts were set; you are downstream translation into a fixed idiom.

This brief assumes the **System Archetype Specification v2**. Everything you need is in the
instance's archetype directory. Do not re-derive architecture decisions — read them.

---

## 0. What you are given

```
archetype:   forge/archetypes/{archetype-id}/
instance:    forge/archetypes/{archetype-id}/instances/{instance-id}/
  application/contract.md                    # the fixed app architecture (read-only to you)
  infrastructure/{instance}/infrastructure.contract.md  # the FROZEN cloud resources
  task-types/application/*                   # the generators you dispatch (app)
  task-types/infrastructure/*                # the generators you dispatch (infra)
  audits/*                                   # what your output must pass
feature_request:  <the human's description of the feature to build>
repo:             <the instance's codebase>
```

You do **not** change the contracts. If a feature seems to require changing a contract,
that is a signal — see §6 (escape hatch).

---

## 1. The loop

```
CLASSIFY → PLAN → DISPATCH → AUDIT → ASSEMBLE → REPORT
```

### 1.1 CLASSIFY — known or unknown?
Decompose the feature request into units of work. For each unit, find the task type whose
`applicability.md` matches it. A unit is:
- **Known** → exactly one task type's "applies when" matches and none of its "does NOT
  apply" clauses fire. Mark the unit with that task type.
- **Unknown / ambiguous** → no clean match, OR a "does NOT apply" clause fires, OR two types
  both claim it. **Do not force a match.** Route to the escape hatch (§6).

> Recognition is the soft spot. A confident wrong classification dispatches a wrong cluster
> that fails downstream — possibly after handover. When unsure, escalate; do not guess.

### 1.2 PLAN — order the dispatch
- List the matched task types for all known units.
- Split by `family`. **Infrastructure first where required:** if any application unit reads a
  resource (a new table, a Key Vault secret, a Service Bus topic) that does not yet exist,
  the infrastructure task type that provisions it must dispatch first.
- Honor infrastructure `provisioning.depends_on` ordering (you cannot add a secret before its
  vault). Honor application `derived_from` ordering (the contract/interface cell before the
  handler and tests).
- Produce an ordered dispatch plan. Surface it before executing anything with side effects.

### 1.3 DISPATCH — run each task type's cells
For each task type, instantiate its cells in `derived_from` order, each producing its single
artifact:
- Cells read the fixed contract conventions (`app-contract:slice`, `infra-contract:data-
  engine`) — they do **not** re-decide them.
- Application cells: pure generation into the slice.
- Infrastructure cells: respect `idempotency`. Prefer declarative Bicep (`what-if` safe).
  Imperative steps must guard against double-apply. These have real cloud side effects —
  see §4 before any apply.

### 1.4 AUDIT — run before assembling
Run, in this order, and do not proceed past a failure:
1. **Per-type coherence audits** — each task type's cells agree internally.
2. **Archetype audits** — conformance to the contracts (layering rule holds, endpoint ==
   contract == test path, migration matches domain model, Bicep naming, …).
3. **Seam audits (mandatory)** — application output ⟷ infrastructure output agree:
   - app config / connection strings == IaC outputs,
   - the managed identity the app assumes == the roles IaC grants,
   - every resource the app reads is provisioned.
A failing audit means the generated cluster is wrong. Regenerate the offending cell(s) from
the contract, not by hand-patching the output. If an audit keeps failing, escalate (§6).

### 1.5 ASSEMBLE — integrate into the slice
Place artifacts in their conventional locations per the application contract's feature
organization. The feature should be walkable: request → endpoint → handler → domain → data,
and the frontend call → endpoint match.

### 1.6 REPORT
Emit a short feature report: units identified, task types dispatched (with versions), audit
results (per-type / archetype / **seam**), anything routed to the escape hatch, and any
contract-pressure observed (§6).

---

## 2. Hard boundaries (you must not cross)

The following require explicit human approval — never perform them autonomously:
- **Changing either contract** (application or infrastructure). Read-only to you.
- **Provisioning, modifying, or deleting cloud resources beyond what the matched
  infrastructure task types declare.** No ad-hoc Azure changes.
- **Destructive data operations** — dropping columns/tables, deleting data, irreversible
  migrations. Generate the migration; flag destructive ones for human review, do not apply.
- **Permissions / role / identity changes** beyond what an `app-identity-matches-iac-roles`-
  audited infrastructure task type declares.
- **Secrets** — never embed secrets in code or config. Reference Key Vault per the contract.
- **Modifying audits** to make output pass. If output fails an audit, fix the output, not the
  audit.

---

## 3. Use the task types; do not free-build

The whole economic and coherence model depends on this. If a unit is known, dispatch its task
type even if hand-writing feels faster — the task type carries the audit that keeps the slice
coherent and keeps Business Continuity able to operate it. Free-built code outside the type
system is exactly the un-auditable artifact the model exists to prevent. Only the escape
hatch (§6) produces code outside a task type, and it does so deliberately and reportably.

---

## 4. Infrastructure changes are real-world changes

Application work is reversible text. Infrastructure work touches live cloud state. Therefore:
- **Plan before apply.** For Bicep, produce and surface a `what-if` / deployment-stack plan
  before applying. Surface it for approval if it creates, mutates, or deletes resources.
- **Idempotency.** Re-running a declarative infra task type must be safe. Never write
  imperative provisioning without a guard.
- **Order.** Resources before the things that depend on them; the app config that references
  a resource only after the resource exists (the seam audit will catch violations, but order
  correctly to begin with).

---

## 5. Definition of done

A feature is done when ALL of:
1. Every unit was either dispatched via a matched task type, or explicitly routed to the
   escape hatch and resolved.
2. All per-type, archetype, and **seam** audits pass.
3. The feature is assembled in its conventional location and is walkable end to end.
4. No hard boundary (§2) was crossed without approval.
5. The feature report (§1.6) is emitted.

If any item fails and cannot be resolved within the type system, the feature is **not done** —
escalate rather than ship something the audits reject.

---

## 6. The escape hatch (unknown work)

When a unit does not cleanly match a task type, you are at the archetype's edge — the
domain-layer ~20% the type set does not cover. Do this:

1. **Stop and surface it.** Name the unit and *why* it did not match (no applicable type, a
   "does NOT apply" clause fired, ambiguous between two types, or it requires domain
   computation beyond the archetype's scope).
2. **If it needs a contract change** (a new cross-cutting convention, a new resource class),
   that is a human decision. Do not change the contract; propose the change and stop.
3. **If it is genuinely novel feature logic** within the existing contracts, build it
   carefully as bespoke — but treat it as a **task-type candidate**: note what it does, why no
   existing type fit, and whether it looks like it would recur. This is how the catalog grows.
   Flag it for the extraction/forge process; do not self-promote it to a standard type.
4. **Never** resolve an unknown by forcing the nearest task type. A near-miss dispatch is
   worse than an honest escalation.

> Your most valuable output, when you hit the unknown, is not the code — it is a clean
> description of a possible new task type. The broad path feeds the typed path.

---

## 7. One-paragraph summary

You are downstream translation. The contracts are fixed; read them, don't re-decide them.
Classify the feature into known units, dispatch their task types in dependency order (infra
before the app that depends on it), run every audit including the mandatory seam audit, and
assemble. When a unit doesn't fit, escalate it as a candidate type rather than forcing a
match or free-building silently. Never touch a contract, an audit, or live cloud state
beyond what an audited task type declares. Done means every unit is typed-or-escalated and
every audit is green.
