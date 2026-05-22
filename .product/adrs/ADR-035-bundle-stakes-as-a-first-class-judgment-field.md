---
id: ADR-035
title: Bundle stakes as a first-class judgment field
status: accepted
features:
- FT-056
- FT-063
- FT-054
supersedes: []
superseded-by: []
domains:
- api
- data-model
scope: cross-cutting
content-hash: sha256:0f32e9abca5e002fadd3cb639a44f59028fd18a82fcc2e75efad7974e0ac8d28
---

## Context

The escalation policy in [ADR-034](ADR-034) relies on a trigger named `stakes_foundational` (and siblings `stakes_elevated`, `stakes_routine`). For that trigger to mean something, *something* in the dispatched bundle has to carry a stakes judgment. There are three candidate locations:

1. **The feature_spec** (a `product-cli` artifact). Reject: feature_specs change phase/status but not stakes; an incremental feature can produce a foundational bundle (e.g. when its blast radius is only visible from the dispatcher's vantage). Coupling stakes to the feature_spec is a category error.
2. **The role binding.** Reject: a role binds to a capability, not to a stake. The same architect role can produce a routine ADR amendment and a foundational ontology change; stakes is per-bundle, not per-role.
3. **The bundle itself.** Accept: the bundle is the unit of dispatch, the unit of audit, and the unit of cost. Stakes is a judgment about *this dispatch* — what is being touched, with how much blast radius, against what existing structure.

The natural follow-up: who decides? Three options:

1. **The dispatcher infers stakes from the artifact's class.** Reject: a regex on artifact type cannot capture "this ontology change is foundational" vs "this ontology change is a typo fix"; the judgment is contextual.
2. **The operator labels every dispatch by hand.** Reject: defeats automation; the meta-loop cannot author bundles if a human must stamp them first.
3. **The role composing the bundle judges stakes.** Accept: the bundle composer (currently `core::bundle::assemble_for_role` for engineering artifacts) has the most context — it sees the focal artifact, the linked ADRs, the diff against prior versions. It is also the right place because the bundle composer already exercises judgment about which ADRs to include, which TCs to cite, what depth to expand.

The risk with option 3 is that "the role" producing the judgment is itself an LLM; LLM-generated stakes labels could be wrong. Mitigation: stakes is *advisory* to the dispatcher (drives escalation triggers); it is not a hard gate. A mislabeled `routine` bundle that is actually foundational will still produce a result; if it is bad, the verifier should catch it and trigger escalation through *other* signals (confidence, audit). Stakes is a fast path for high-blast-radius cases, not a load-bearing safety check.

See the parent PRD: §7 (bundle stakes), §11.2 (acceptance criteria touching `bundle.stakes`).

## Decision

Add a `dec:stakes` datatype property to `dec:Bundle` with a closed enumeration of three values: `routine`, `elevated`, `foundational`. The field is required (SHACL `sh:minCount 1`); bundles without an explicit value default to `routine` at composition time. See [FT-056](FT-056).

### Value semantics

- **`routine`** — Standard work within established patterns. Incremental feature_spec, bugfix ADR, normal verifier dispatch. Default for every bundle unless the composer overrides it.
- **`elevated`** — Nontrivial downstream blast radius. Cross-cutting refactor, schema change, new role binding addition, change touching > 3 systems.
- **`foundational`** — Touches the framework itself or its long-lived structures. Ontology change, new artifact type, meta-loop policy change, change to the orchestration system itself. Capability or RoleBinding catalog edits are by definition `foundational`.

### Who sets it

The role that composes the bundle. For engineering work, `core::bundle::assemble_for_role` reads the focal artifact and applies a default ladder:

- Focal artifact is a `dec:Capability`, `dec:RoleBinding`, ontology change, or new artifact-type definition → `foundational`.
- Focal artifact is a cross-cutting ADR or a feature_spec linked to ≥ 2 cross-cutting ADRs → `elevated`.
- Otherwise → `routine`.

This ladder is the *default*; the bundle composer (and, downstream, the meta-loop) can override. When unclear, prefer `routine` — the cost of a missed escalation is one cheap attempt that fails verification; the cost of false-positive `foundational` everywhere is wasted Opus calls.

### What it does, what it doesn't

- **Drives `stakes_*` escalation triggers** ([ADR-034](ADR-034)). This is the primary effect.
- **Drives `reasoning_effort` parameter mapping** ([FT-063](FT-063)) for capabilities marked `configurable_effort = true`. Stakes maps `routine → low`, `elevated → medium`, `foundational → high`.
- **Does NOT gate dispatch.** A `foundational` bundle still dispatches; it just produces different escalation behavior.
- **Does NOT change the worker contract.** Workers see the stakes value (it is part of the bundle), but no current role behavior is contingent on it — only the dispatcher's escalation logic and parameter computation.

## Consequences

**Positive.**

- Stakes is a single per-bundle field, easy to inspect and audit. `dec bundle show <hash>` surfaces it.
- The default ladder catches the common cases (ontology changes → foundational) without requiring per-dispatch judgment.
- Stakes composes cleanly with confidence and audit triggers — escalation policy can say "escalate on `stakes_foundational` *or* `confidence_below_0.5`" and both are evaluated against the same signal object.

**Negative / accepted costs.**

- LLM-composed bundles can mislabel stakes. The mitigation (it is advisory, not gating) is correct in principle but means a foundational change can slip through the routine path until verification catches it.
- Three values is a coarse scale. There is no `urgent` or `experimental`; future scenarios may want a richer ontology. Adding values requires extending the SHACL enum and the trigger vocabulary together — a small but real coupling.
- The default ladder lives in `core::bundle::assemble_for_role`, which means a new artifact type's default stakes is part of the artifact-type integration work, not a free-floating policy.

**Boundary enforcement.**

- SHACL on `dec:Bundle` requires `dec:stakes` ∈ {routine, elevated, foundational} (enum constraint via `sh:in`).
- The bundle composer is the only writer; workers do not modify stakes.
- The dispatcher reads stakes via `bundle.stakes`; it does not infer or override.

## Status

Proposed. Governs [FT-056](FT-056) (Bundle.stakes schema), [FT-063](FT-063) (reasoning_effort mapping). Companion to [ADR-034](ADR-034) (escalation triggers consume stakes).
