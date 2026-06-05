---
id: ADR-085
title: Archetype status promotion candidate → standard is a gated human decision with evidence requirements
status: accepted
features:
- FT-147
- FT-156
- FT-158
supersedes: []
superseded-by: []
domains:
- api
- data-model
scope: platform
content-hash: sha256:37f08e3c3806d4a0a136e2cf1954614200c81341994a2031c443dba614a991ec
---

**Status:** Proposed

## Context

[ADR-082](ADR-082) introduces `dec:Archetype` as a graph-resident artifact with a status field. The valid values are `candidate`, `standard`, and `quarantined`. The pattern-extraction playbook (`briefs/pattern-extraction-playbook-v2.md §9.1`) is direct: **the broad worker emits only `candidate`. Promotion to `standard` is always a gated human decision.** The hard rule sits in `briefs/system-archetype-spec-v2.md §11.11` for the same reason.

The catalog has two paths to gain an archetype:

- **Mining** — the broad worker reads N instances of a recurring system kind and emits a candidate archetype with its two contracts, TaskType set, and audits. The playbook's hand-back report (`§8`) is the evidence of what was mined.
- **Direct authoring** — a human writes the archetype manifest, contracts, TaskTypes, and audits in `forge/archetypes/{archetype-id}/` and the catalog ingests them.

Both paths produce `status: candidate`. Neither path produces `status: standard` automatically. The decision is whether the path from `candidate` to `standard` is automatic (`audit catches a drift → promote`), advisory-with-human-approval, or strictly human-gated with evidence requirements.

Promotion to `standard` carries real consequences downstream:

- `standard` archetypes are recommended by the catalog for new customer engagements ("use the Self-Service Portal archetype"). A wrong recommendation costs customer trust and engineering rework.
- `standard` archetypes have their seam audits' `monolith_bar: passes` claims trusted by the dispatcher; TaskTypes route through the cluster-dispatch path rather than the broad-worker escape hatch. A wrong trust dispatches confidently-wrong clusters at scale.
- `standard` archetypes mark the boundary between "we have seen this pattern enough times to commit to it" and "this is a candidate we are still validating". The catalog's evidence-honesty per the playbook (`§7`, `§8`) depends on this distinction being meaningful.

If promotion is automatic on the first seam-audit catch, a single accidental drift detection minted by a broad-worker run elevates a candidate to standard. If promotion is a free-form human decision without evidence requirements, the distinction reduces to "whichever maintainer felt like it that day". Neither produces a `standard` set the catalog can lean on.

The catalog already gates an analogous transition: ADR status `proposed → accepted` is CLI-only (per [ADR-032](ADR-032) and enforced by E020) and requires SHACL conformance. The archetype-promotion gate is the same shape applied at the archetype layer.

## Decision

**Archetype `status: candidate → standard` promotion is a strictly human-gated decision requiring: (1) evidence of multiple instances; (2) all seam audits at `monolith_bar: passes` with regression evidence; (3) coverage honesty in EVIDENCE.md; (4) explicit reviewer sign-off recorded as an `ArchetypePromotion` artifact. Automatic promotion is forbidden. Demotion (`standard → candidate`) is human-gated with the same shape but lower evidence bar.**

### 1. Promotion has four mandatory evidence requirements

A promotion request is rejected unless all four hold:

1. **Instance evidence (≥3 known-good instances).** The archetype declares at least three `Instance` artifacts (real systems on this archetype with a recorded repo + commit). Three is the minimum — pattern-extraction's threshold ("≥3 occurrences across ≥2 instances" for a TaskType candidate) lifted up to the archetype layer. Below three, the variance estimate is unreliable.
2. **Seam audit strength (every SeamAudit at `monolith_bar: passes`).** Per [ADR-084](ADR-084), an archetype with any `candidate-audit-weak` or `unrunnable` seam audit cannot promote. The `passes` claim is the gating mechanism; this ADR enforces it at promotion time.
3. **Coverage honesty (EVIDENCE.md fields filled).** The archetype's `EVIDENCE.md` declares: `archetype_layer_estimate` (the fraction of THIS ARCHETYPE's known features the TaskType set ships), `application_contract_held_invariant` (boolean — did the application contract hold across all instances?), `instance_variance` (`low | medium | high`), `seam_regression_results` (links to RegressionEvidence artifacts per audit), and `coverage_note` (free-text caveats). Missing fields block promotion.
4. **Application contract invariance proven.** `application_contract_held_invariant: true` is required. If any instance forced an application contract change, the archetype boundary is wrong (per playbook `§7`); the promotion is rejected with a diagnostic pointing at the boundary disagreement.

### 2. Promotion request is itself a graph artifact

A new artifact type `dec:ArchetypePromotion` records the request:

- `archetype: <archetype_id>` — the archetype being promoted.
- `requested_status: standard` — only `standard` is a valid target (no auto-demotion via this path).
- `evidence: { instances: [...], seam_audits: [...], regression_records: [...], coverage_note: String }` — the four mandatory pieces, denormalised onto the promotion record so the audit trail survives later mutations.
- `reviewer: String | required` — the human reviewer's identity (matching the CLI principal; not yet first-class but persistable as a string).
- `decision: pending | approved | rejected` — the reviewer's verdict.
- `decision_reason: String | required when decision != pending` — free-text justification.
- `decided_at: ISO8601 | required when decision != pending` — the decision timestamp.

The promotion record is immutable once `decision` is non-pending (the SHACL chokepoint refuses mutations on a non-pending promotion). Re-promoting a demoted archetype creates a *new* ArchetypePromotion record, building an audit chain.

### 3. The promotion command is CLI-only

`dec archetype promote <archetype_id> --reviewer <name> --reason <reason>` is the only path to mutate `Archetype.status` from `candidate` to `standard`. The MCP surface intentionally does not expose this — promotion is a human decision, not an automated one, and the MCP path would be the wrong shape if it routed through an agent. This mirrors [ADR-032](ADR-032)'s decision to keep ADR `accepted` CLI-only.

The command:

1. Reads the archetype.
2. Checks the four evidence requirements (§1).
3. Refuses with E110 (`E110_ArchetypePromotionEvidenceMissing`) listing which checks failed.
4. On pass, creates the `ArchetypePromotion { decision: approved, ... }` record and flips `Archetype.status: standard`.
5. Emits a `dec:ArchetypePromoted` event.

A separate `dec archetype reject-promotion <archetype_id> --reviewer <name> --reason <reason>` path creates an `ArchetypePromotion { decision: rejected, ... }` record without mutating the archetype status. This records evidence of *why* a promotion was rejected; future re-promotion attempts cite the prior rejection record.

### 4. Demotion is symmetric but lower-bar

`dec archetype demote <archetype_id> --reviewer <name> --reason <reason>` flips `standard → candidate`. Evidence requirements are lower (the reviewer's reason field is the only required input) because demotion is the safety valve: a `standard` archetype found to have a regression, a contract drift, or a customer-driven invariant violation needs to drop fast. The audit trail is the same shape (an `ArchetypePromotion { requested_status: candidate, decision: approved, ... }` record), but the evidence checks are skipped.

Quarantining (`standard | candidate → quarantined`) is automatic per [ADR-084](ADR-084) §1 when a seam audit becomes `unrunnable`. Quarantine recovery to `candidate` requires the same shape as demotion (CLI, reviewer, reason).

### 5. Standing diagnostic — `product graph check` surfaces promotion-eligible archetypes

`product graph check` reports (W104) every archetype where the four evidence requirements hold but `status` is still `candidate`. This makes the gate visible without forcing promotion — the archetype is ready, the catalog flags it, a human can decide when to promote. The warning is informational; no enforcement.

### 6. No automatic promotion paths

No code path mutates `Archetype.status: standard` other than the CLI command above. The SHACL chokepoint at GraphWriter ([ADR-041](ADR-041)) enforces this with E020-style rejection for any other mutation attempt. The classifier, the dispatcher, the seam-audit runner, the pattern-extraction worker, and the regression-test framework can all *read* `Archetype.status` but cannot write it.

## Rejected alternatives

### Auto-promote when all seam audits reach `monolith_bar: passes`

Trigger promotion automatically when the last `candidate-audit-weak` audit gets evidence. Rejected — the playbook's hard rule (`§9.1`) is explicit that promotion is human-gated, and the rationale is precisely that an audit-strength signal is not a multi-instance-validation signal. An archetype could have strong seam audits with only one instance and no variance data; auto-promoting it lies about the catalog's evidence quality.

### Auto-promote when the archetype has been used in N customer engagements

Trigger promotion on usage volume (e.g., three approved customer instances). Rejected — usage volume is a noisy proxy for archetype quality. A poorly authored archetype could be used three times if no one notices the seams; auto-promotion would lock in the badness. The instance count is one of four evidence requirements, not the sole signal.

### Promotion is a free-form human decision with no evidence requirements

`dec archetype promote` requires only `--reviewer` + `--reason`. Rejected — reproduces the "whichever maintainer felt like it" problem. The evidence requirements are not bureaucracy; they encode the playbook's `§7` regression-test requirement, the spec's coverage-honesty rule, and ADR-084's monolith bar into structurally-checkable preconditions. The reviewer's judgment is *over and above* the evidence, not instead of it.

### Promotion is gated by `product verify --platform` rather than at promotion time

Add the four evidence requirements as cross-cutting TCs that run continuously. Rejected — wrong shape. `product verify --platform` runs continuously over the catalog state; promotion is an event. Continuous verification of "every standard archetype still has its four evidence pieces" is fine and lands separately as a maintenance TC. The *promotion* gate (do not let `candidate` become `standard` without evidence) is event-driven and belongs at the mutation site.

### Multi-reviewer approval for promotion

Require ≥2 reviewers to approve a promotion. Rejected for v1 — adds coordination cost without proven benefit at the current catalog scale (one archetype, the decision-cli self-implementation one). Lands as a follow-on once the catalog has enough archetypes that the single-reviewer model shows weakness; the `ArchetypePromotion` record's structure leaves room for multiple `reviewer` entries without schema break.

### Skip the demotion path; force a re-mint instead

If a `standard` archetype has a regression, abandon it and mint a new archetype with a different id. Rejected — wasteful and confusing. The archetype is recognisable across instances; demoting it to `candidate` while the regression is investigated is the right operational shape. Re-minting destroys the audit trail; demotion preserves it.

### Hide the warning W104; surface only on explicit query

Do not have `product graph check` proactively suggest promotion-ready archetypes. Rejected — visibility is a feature. The warning is informational, not enforcing; it costs nothing and prompts the human review the playbook expects.

## Consequences

### Positive

- **The `standard` claim becomes meaningful.** Three instances, every seam audit at `passes`, contract invariance proven, coverage honesty documented, human reviewer sign-off recorded. The catalog can lean on `standard` archetypes for customer recommendations without that being a bet on autopilot.
- **The audit trail is graph-resident.** `ArchetypePromotion` records persist promotion and rejection history. A customer asking "how did this archetype reach `standard`?" gets a concrete record with reviewer, reason, evidence links, and timestamp.
- **Demotion is fast.** When a `standard` archetype has a regression in production, a single CLI command demotes it and dispatches against it route to the broad worker until the issue is resolved. No promotion-process inversion required.
- **Pattern-extraction stays principled.** Mining produces only `candidate`s; promotion takes deliberate human review. The broad worker's role as explorer-and-typifier (per ADR-080 and the playbook) is preserved at the archetype layer too.
- **The gate's strength is structural, not procedural.** A maintainer cannot accidentally promote by clicking the wrong button — the CLI enforces the four checks, and the MCP path does not expose the verb at all.

### Negative / accepted trade-offs

- **Bootstrap pressure on the first archetype.** The decision-cli self-implementation archetype ([FT-160](FT-160)) is a new mint; reaching three instances requires either using it for a real engagement (which we are not, in v1) or counting the live decision-cli repo itself plus historical snapshots. The archetype will live in `candidate` for a long time; that is correct, not a failure.
- **Promotion latency.** Even when an archetype is evidence-ready, the human review step adds calendar time. Mitigated by W104 surfacing readiness; the cost is real but is paid against the safety property of the `standard` set.
- **The `ArchetypePromotion` audit trail is verbose.** Multiple promote/demote cycles produce many records. Accepted — the verbosity is the audit trail; pruning would defeat it.
- **W104 churn.** A new archetype with growing evidence will trip W104 every check until promoted. Operators tune this out. Accepted — explicit acknowledgement that a warning is informational lives in the graph-check output's classification.
- **Demotion can be abused as a workaround.** A maintainer pressed for time could demote a `standard` archetype to skirt the gate that would otherwise block a change. Mitigated by the audit trail — every demotion records a reason, and a pattern of bypass demotions is detectable by reviewing `ArchetypePromotion` records.

### Relationship to prior decisions

- **[ADR-082](ADR-082)** introduces `Archetype.status` as the gated field; this ADR governs the transitions.
- **[ADR-084](ADR-084)** sets the seam-audit monolith bar; promotion uses `monolith_bar: passes` as one of its evidence requirements.
- **[ADR-032](ADR-032)** kept ADR `accepted` CLI-only with SHACL enforcement; this ADR mirrors that shape at the archetype layer (`dec archetype promote` is CLI-only; SHACL E020-style rejection elsewhere).
- **[ADR-041](ADR-041)** placed SHACL enforcement at the GraphWriter chokepoint; this ADR's E110 check (evidence missing) lands there alongside the existing artifact-shape checks.
- **[ADR-014](ADR-014)** governs cross-cutting fitness functions as ADRs + TCs; the maintenance check ("every `standard` archetype still has its four evidence pieces") lands as a cross-cutting TC under this rule.

## Status

Proposed. Promotes to accepted once FT-147 (Archetype artifact type with status field), FT-153 (audit pipeline gating on `not-safely-dispatchable` weak audits), and `dec archetype promote / demote` CLI commands ship, the GraphWriter rejects automated `Archetype.status: standard` mutations with E020-style errors, and W104 surfaces a promotion-ready archetype in `product graph check` output.
