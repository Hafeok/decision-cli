---
id: ADR-084
title: Seam audit is mandatory and load-bearing — no archetype ships without it
status: accepted
features:
- FT-147
- FT-152
- FT-153
- FT-157
- FT-160
supersedes: []
superseded-by: []
domains:
- api
- data-model
- observability
- security
scope: cross-cutting
content-hash: sha256:d2968095320aa31dde6e44ec8ed089a5333105430743c18562e037c4d9d8d9fa
---

**Status:** Proposed

## Context

Under [ADR-082](ADR-082), an archetype owns audits at three scopes: per-type coherence (the [FT-139](FT-139) audit, owned by a TaskType), archetype conformance (the cells of any dispatched work conform to a contract), and **seam audit** (application output ⟷ infrastructure output agree). The first two scopes have witnesses in the system already; the seam audit is new in v2 and is the load-bearing detail the spec is built around (`briefs/system-archetype-spec-v2.md §8`).

The seam audit catches a specific failure class:

- App config / connection strings the application expects do not match what IaC emits.
- The managed identity the application assumes does not match the roles IaC grants.
- Resources the application reads (a Key Vault secret, a Service Bus topic) are not provisioned by infrastructure.

These are the failures that surface *after* handover — the deployment "succeeds" because IaC ran and the application starts, then a request hits the path that needs the Key Vault secret and the worker dies with a 403 the operator has to debug under SLA pressure. They are precisely the kind of bug a single broad agent reading both the application and the infrastructure in one context would have caught for free, because the contradiction is visible from inside the shared context. The decomposition into separate application and infrastructure TaskTypes loses that free coherence. The seam audit puts it back.

The witnessed seam inside decision-cli already exists. Code-writer assumes `LITELLM_BASE_URL` at runtime ([ADR-053](ADR-053), [ADR-064](ADR-064)). The LiteLLM-proxy worker-distribution slice ([FT-096](FT-096)) provisions it. Nothing in the type system today asserts these two facts agree — the agreement is held by a human reading both. If `FT-096`'s deployment changed the env-var name to `LITELLM_PROXY_URL` and the corresponding code-writer commit lagged a day, every dispatch would fail after the deploy with an opaque connection error. Once `dec drive ship --all` is the production path, that gap is no longer acceptable. The seam audit is the structural fix.

The spec is direct about the bar (`§8`, lifted from DDD core):

> **Every audit must be at least as strong as what a single broad agent gets for free from shared context. If it is weaker, the decomposition is worse than the monolith for that concern.**

This is non-negotiable for the seam audit specifically. An archetype with a seam-audit slot that is empty, or filled with audits weaker than the monolith bar, has worse properties than just routing every feature through the broad code-writer — because the type system says "this is fine" while the bug class the layer exists to catch goes uncaught. That outcome is strictly worse than the pre-archetype baseline.

## Decision

**No archetype ships with `status: candidate` (let alone `standard`) without a non-empty seam-audit set that meets the monolith bar. The seam audit is the load-bearing audit of the archetype layer; without it the decomposition is unsafe by construction.**

### 1. Mandatory non-empty seam-audit set per archetype

The `Archetype` artifact's `seam_audits: [SeamAudit]` field must be non-empty for the archetype to register as anything other than `quarantined`. An archetype registered without seam audits is rejected at GraphWriter SHACL chokepoint with E102 (`E102_ArchetypeMissingSeamAudits`). The check is structural — counting the linked SeamAudit artifacts — and runs at every archetype mutation.

This is asymmetric with archetype audits and per-type coherence audits, which are also expected but not gated by E102. The seam audit is singled out because it is the audit whose absence makes the decomposition strictly worse than the broad-worker baseline; the others strengthen the type system but their absence is recoverable (the broad worker would have caught the coherence violation too). The seam audit's absence is unrecoverable — the broad worker's free coherence over a shared context is the only thing that catches the seam class today, and the decomposition removes that context.

### 2. The monolith bar — every seam audit must meet it

Each SeamAudit declares `monolith_bar: passes | candidate-audit-weak | unrunnable` plus a `monolith_bar_evidence` field with a free-text justification (or a link to a regression-test record produced under `pattern-extraction-playbook-v2.md §7`). The `passes` value is set by a human reviewer after evaluating evidence; it cannot be auto-set.

`monolith_bar: candidate-audit-weak` means the audit is in the archetype but its strength is unproven. TaskTypes whose conformance to the archetype's contracts depends on a `candidate-audit-weak` seam audit are flagged `not-safely-dispatchable` and the dispatcher refuses to dispatch them ([FT-153](FT-153)). The escape hatch (the broad worker) absorbs the dispatch instead. The archetype itself can still register as `candidate` with a weak audit — the slack is so the slice can land without blocking — but the archetype cannot promote to `standard` while any seam audit is `candidate-audit-weak` (per [ADR-085](ADR-085)).

`monolith_bar: unrunnable` means the audit's enforcement script is missing or broken. Same downstream effect as `candidate-audit-weak`, but additionally the archetype's own `status` is forced to `quarantined` until the audit is restored.

### 3. Required seam-audit families

Three seam-audit families are required for every archetype that has any application + infrastructure split (which is every archetype under ADR-082). Each family has at least one concrete audit:

| Family | What it asserts | Mechanical check |
|---|---|---|
| **`app-config-matches-iac-outputs`** | Every config value the application expects (env var, connection string, endpoint URL) appears as an output of the infrastructure's IaC, with the same name and a compatible value shape. | Parse application's config reads (env-var lookups, configuration-section reads); parse IaC's outputs; assert the application's read set is a subset of the IaC's output set; assert value-shape compatibility (URL is URL, JSON is JSON). |
| **`app-identity-matches-iac-roles`** | The managed identity the application assumes (federated identity client ID, Service Principal, workload identity) is granted the union of all roles the application's code requires (RBAC scopes, Key Vault access policies, Storage roles). | Parse application's identity assumption; parse application's resource access patterns; parse IaC's role assignments to that identity; assert every required role is granted. |
| **`app-resource-expectations-met`** | Every infrastructure resource the application reads or writes — Key Vault secret, Service Bus topic, Storage container, SQL table catalog entry — is provisioned by infrastructure TaskTypes that have dispatched and audited green. | Build the application's resource-reference set from cell outputs; build the infrastructure's resource-creation set from IaC outputs; assert the application's set is a subset of the infrastructure's set. |

An archetype may add additional seam-audit families beyond these three (e.g., network-policy alignment, observability stack agreement); the three are the minimum.

### 4. Seam-audit runner is a first-class component

The seam-audit runner ([FT-160](FT-160) lands the first implementation) is a separate component from the per-type coherence audit and the archetype conformance audits. It runs as the third stage of the cluster-dispatch audit pipeline (per-type → archetype → seam, fail-fast — [FT-153](FT-153)). It reads the union of all cell outputs from the dispatched cluster — application *and* infrastructure cells — and runs every SeamAudit's enforcement script against that union. Any failure aborts the assembly stage; the worktree edits are rolled back; the outcome is `SeamAuditFailed { audit_id, family, detail }`.

The runner is reusable across archetypes. Each Archetype's `seam_audits` field references SeamAudit artifacts that declare a `runner` + `runner-args` pair (the same shape as test-criterion runners under [ADR-013](ADR-013) and [ADR-014](ADR-014)). The script lives under `forge/archetypes/{archetype-id}/audits/seam/` and is shipped with the archetype, not the binary.

### 5. Regression-test evidence is required for monolith-bar `passes`

The pattern-extraction playbook (`§7`) requires regenerating a known-good instance via dispatch and running the seam audits against the regenerated output. A seam audit can claim `monolith_bar: passes` only when its `monolith_bar_evidence` references a regression test record showing the audit caught at least one drift the type system would otherwise have missed (the load-bearing-prototype standard ADR-082 §Status set). The evidence record is graph-resident — a `RegressionEvidence` artifact linked from the SeamAudit — so the claim is auditable, not asserted.

Without that evidence, the monolith bar is unproven and the audit stays `candidate-audit-weak` regardless of how confident the author is.

## Rejected alternatives

### Make seam audits a best-practice recommendation

Ship the SeamAudit artifact type, recommend authoring at least one per archetype, but do not gate registration. Rejected — exactly the failure mode the briefs warn against (`§8`, "this is the most important detail in v2"). A recommendation is not a property. The catalog's safety guarantee is "an archetype that registers will catch handover bugs"; a soft recommendation means an archetype could register and not catch them. The whole point of the layer fails open instead of closed.

### Require a seam audit but allow `monolith_bar: candidate-audit-weak` indefinitely

Gate registration but not promotion. Rejected for archetypes wanting `standard`; accepted for archetypes wanting `candidate`. The compromise (accept `candidate-audit-weak` at `status: candidate`, require `passes` for promotion to `standard`) is exactly what the Decision §2 records. Stronger gates would block the broad-worker mining path from minting anything; weaker gates would let `standard` archetypes ship without proven audit teeth.

### Gate at archetype dispatch, not at archetype registration

Register archetypes freely; refuse to dispatch features against an archetype with no seam audits. Rejected — the broken state is now stored in the catalog and surfaces as a dispatch-time error rather than a registration-time error. The cost paid at registration is borne once per archetype; the cost paid at dispatch is borne every time a customer tries to use the archetype. Move the cost earlier.

### Combine seam audits with archetype audits — one scope, not two

Drop the three-scope split; let archetype audits assert both contract conformance and seam coherence. Rejected per ADR-082 §4 — the audit scopes are intentionally separated because the failure classes are different and the audit authors are different. An archetype conformance audit is owned by the archetype author (checking the contract is obeyed); a seam audit is owned by the integration boundary between the application and infrastructure TaskType families. Conflating them dilutes both.

### Allow non-empty seam audits but skip the monolith-bar check

Require at least one SeamAudit per archetype but trust the author's strength claim. Rejected — the monolith bar is the whole point. An audit that does not catch what a broad agent would catch for free is worse than no audit (false confidence). The bar exists; gating against it is the whole load-bearing claim.

### Defer the gate until decision-cli's own self-implementation archetype has been mined

Land the SeamAudit artifact type without the registration gate; require gating once the first non-decision-cli archetype lands. Rejected — decision-cli's self-implementation archetype is the test of whether the gate is workable. If FT-160's seam audit cannot meet the monolith bar in finite time, that is a diagnostic about either the framework (the contract format is incomplete) or the archetype (the witnessed drift cannot be mechanically described). Either is worth knowing before a second archetype lands. The gate from day one surfaces the diagnostic.

## Consequences

### Positive

- **The decomposition's safety property is structural.** An archetype that registers catches the seam class. The promise to Business Continuity is enforced, not aspirational.
- **The first archetype's seam audit becomes the prototype.** [FT-160](FT-160) lands `app-config-matches-iac-outputs` against the witnessed LiteLLM-proxy / code-writer drift; the audit either catches it (validates the framework) or cannot (signals the framework needs work). Either outcome is informative.
- **Audit authors have a clear bar.** "Did it catch what the broad worker would have caught?" is a single question with a yes/no answer and a regression-test record as evidence.
- **Promotion to `standard` becomes meaningful.** `standard` archetypes have proven seam audits with regression evidence; `candidate` archetypes carry weak-audit flags openly. The two statuses now mean materially different things.
- **The dispatcher fails safe.** TaskTypes with weak-audit dependencies route to the broad worker (the explicit escape hatch). No silent dispatch of clusters whose safety is unproven.

### Negative / accepted trade-offs

- **Archetype authoring is slower.** Writing a SeamAudit that passes the monolith bar requires structurally describing a class of drift and writing an enforcement script. Easier than the broad worker's free coherence ("just read both"), harder than nothing.
- **Regression evidence requires a known-good instance.** A new archetype with no known-good instance cannot claim `passes` on any seam audit. This is correct — the playbook explicitly requires the regression test — but means greenfield archetypes start in `candidate` and stay there until evidence accumulates.
- **The runner becomes a critical path.** Bugs in the seam-audit runner (false negatives) silently break the gate. Mitigated by per-TC coverage of the runner itself in FT-160 — the runner has its own TCs that exercise the three required audit families.
- **A `quarantined` archetype is a real state.** The catalog now has an archetype state ("seam audit unrunnable") that operators have to recognise and triage. The pre-archetype baseline had no analogue. Documentation cost paid once.
- **Some witnessed drifts will not survive the bar.** The LiteLLM-proxy / code-writer drift may turn out to require a richer config-output match than `app-config-matches-iac-outputs`'s v1 implementation can express. If so, the audit lands as `candidate-audit-weak` and FT-160 ships an extension before promotion. The diagnostic is the point.

### Relationship to prior decisions

- **[ADR-082](ADR-082)** introduces the three audit scopes; this ADR makes the seam scope's non-emptiness load-bearing.
- **[ADR-085](ADR-085)** governs archetype `candidate → standard` promotion; this ADR provides one of the gates (no archetype with `monolith_bar: candidate-audit-weak` audits promotes).
- **[ADR-014](ADR-014)** established cross-cutting fitness functions as ADRs + TCs. Seam audits are per-archetype fitness functions; they live inside `forge/archetypes/{archetype-id}/audits/seam/` rather than `.product/`, but the principle (graph-resident, runner-driven, machine-checked) is the same.
- **[ADR-041](ADR-041)** placed SHACL enforcement at the GraphWriter chokepoint. The E102 check for empty seam-audit sets lands there, the same way every other artifact-shape SHACL constraint does.
- **[FT-096](FT-096) / [ADR-053](ADR-053) / [ADR-064](ADR-064)** ship the witnessed seam: code-writer expects `LITELLM_BASE_URL`; the LiteLLM-proxy slice provisions it. This ADR's first concrete audit (FT-160) makes that agreement structural.

## Status

Proposed. Promotes to accepted once FT-152 (SeamAudit artifact type), FT-153 (audit pipeline), and FT-160 (first archetype's seam-audit set) ship, the GraphWriter rejects an archetype with empty seam audits via E102, and the first seam audit ([FT-160](FT-160)'s `app-config-matches-iac-outputs`) demonstrably catches the witnessed LiteLLM-proxy / code-writer drift in a regression test against the live repo — the same monolith-bar standard this ADR is gating against.
