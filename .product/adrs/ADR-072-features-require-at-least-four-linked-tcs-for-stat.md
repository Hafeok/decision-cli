---
id: ADR-072
title: Features require at least four linked TCs for status complete
status: accepted
features:
- FT-126
- FT-127
- FT-128
- FT-129
- FT-130
- FT-131
- FT-132
- FT-133
- FT-138
supersedes: []
superseded-by: []
domains:
- api
- observability
scope: cross-cutting
content-hash: sha256:5b74cd5f4e1b1a30fed117c1341665cd8a4b2f5820feaedf055b112da7022f53
source-files:
- scripts/checks/feature-tc-coverage.baseline
- scripts/checks/feature-tc-coverage.sh
---

**Status:** Proposed

**Context:**

The current completion contract has two seams that drift apart in practice:

1. `product verify FT-XXX` runs every TC linked to a feature and is the documented "definition of done" (CLAUDE.md).
2. `product feature status FT-XXX complete` flips the status field with no verification gate. Headless dispatches and human shortcuts both reach for it.

Today, **92 of 119 complete features (77%) have fewer than 4 linked TCs**, and **53 have exactly one**. A feature with one TC means `product verify FT-XXX` exercises a single behavioural assertion — typically the happy path. Edge cases, error paths, integration with adjacent features, and side-effect/state assertions all go unvalidated, but the feature is nonetheless reported as "complete."

For LLM-driven implementation this is a context-bundle problem, not just a coverage problem. The implementer reads the linked TCs as the operational definition of the feature. One TC frames the work as one behaviour; four TCs frame it as a behavioural envelope. Agents pattern-match against what they see in the bundle — a single TC produces narrowly-fitting code that breaks on the first adjacent assumption.

The same principle applies to verification trust. A green verify on a one-TC feature is information-poor; a green verify on a four-TC feature distinguishes between scenarios. This ADR raises the floor.

**Decision:**

A feature is eligible for `status: complete` only when its count of linked TCs meets or exceeds a **configurable floor**, declared in `.dec/config.toml` as:

```toml
[verification]
min_tcs_per_feature = 4   # built-in default; see Rationale → "Why four as the default"
```

The value is resolved through ADR-068's precedence chain (`--flag` > `DEC_VERIFICATION_MIN_TCS_PER_FEATURE` env > `.dec/config.toml` > built-in default of 4). The default ships at 4. Teams that want a stricter floor raise it in their `.dec/config.toml`; teams operating in a slice that legitimately needs a softer floor lower it explicitly and own the reduced verification trust. Either way, the choice is reviewable in the repo, not buried in a script constant.

The rule applies at two enforcement points:

1. **Graph-health check** — `scripts/checks/feature-tc-coverage.sh` (a `runner: bash` TC validated by this ADR) reads the threshold via ADR-068's precedence chain and fails CI when any feature with `status: complete` has fewer than `min_tcs_per_feature` entries under `tests:`.
2. **Status-transition gate** *(deferred to a follow-up feature_spec)* — `product feature status FT-XXX complete` should refuse the flip when the count is below the configured floor. Until that lands, the CI check is the gate; the rule is enforced reactively in PRs rather than proactively in the CLI.

### What counts

A "linked TC" is an entry under `tests:` in the feature's front-matter whose corresponding TC file exists and is not in `status: superseded` or `status: abandoned`. Quantity is the rule; this ADR does not mandate specific coverage axes. The recommended (non-enforced) breakdown is:

- **Happy path** — primary intended behaviour with realistic inputs.
- **Edge / failure path** — boundary conditions, malformed input, error propagation.
- **Integration** — interaction with at least one adjacent feature or cross-cutting ADR.
- **State / observability** — assertion on persisted state, emitted events, graph mutations, or other side effects.

The four axes are a thinking aid for authors at the default floor of 4. Mechanical enforcement only counts entries; gaming the count with four trivial TCs is technically permitted but is visible to a human reviewer and to `product preflight`, which surfaces domain-coverage gaps independently.

### Why four as the default

- **One** is insufficient — a single TC pins behaviour at one point and provides no signal that the feature has been exercised against contrast cases. This is the current modal value (53/119 features) and is the failure mode this ADR targets.
- **Two or three** is improvement but typically allows happy + sad without exercising integration or state.
- **Four** is the smallest count that practically forces an author to think about multiple coverage axes. It matches the recommended axis breakdown above.
- **More than four** risks busy-work and trivial-TC gaming. Four is a floor, not a ceiling — features that genuinely need more (FT-008 has 12, FT-114 has 9) are not constrained.

Teams that need a different default (a research spike with a relaxed floor of 2, a security-critical product line with a strict floor of 6) raise or lower `[verification] min_tcs_per_feature` in `.dec/config.toml`. The default of 4 is the recommended starting point for general engineering work.

### Backfill

The 92 pre-existing under-covered features are pre-existing technical debt measured against the default floor of 4. Two failure modes to avoid:

- **Hard-fail CI immediately** — blocks every PR until backfill is done. Unrealistic given the volume.
- **Grandfather indefinitely** — preserves the failure mode the rule is designed to eliminate.

The chosen ramp:

1. The CI check runs from this ADR forward. It reports under-covered features on stdout but **exits 0** (clean) when the only violations are pre-existing.
2. Pre-existing violations are recorded in a baseline file `scripts/checks/feature-tc-coverage.baseline` (one feature ID per line) at ADR-072 acceptance. The check fails (exit 1) when a feature **not in the baseline** has fewer than `min_tcs_per_feature` TCs, or when a feature **in the baseline** has lost TCs (regression).
3. A follow-up feature_spec (to be authored separately) introduces a **TC-author worker** — a dispatchable role that consumes a feature bundle and produces TC drafts via `product test new` + `product test runner` for under-covered features. The baseline file is depleted as that worker runs.
4. The status-transition gate (see "Decision" point 2) lands once the baseline is empty.

The baseline is measured against the *default* floor (4). If a team raises the configured floor to 6 in `.dec/config.toml`, features at 4 or 5 TCs (currently passing) become new violations and are added to the baseline at that point — the baseline is a snapshot, not an exemption from the operator's chosen threshold.

This staged enforcement keeps the rule mechanically enforced on new work while admitting that retrofitting 92 features is a separate operational effort with its own feature_spec and rollout.

### Lifecycle (interaction with existing flows)

- **Feature authoring** — `product feature new` is unchanged. The expectation that authoring includes (or schedules) at least `min_tcs_per_feature` TCs is documented in CLAUDE.md.
- **Implementation** — `dec implement` and `product implement` are unchanged. The implementer reads however many TCs are linked.
- **Completion** — `product verify FT-XXX` is unchanged (it already runs every linked TC). The CI step `product verify --platform` includes the coverage TC validated by this ADR; a feature with `status: complete` and fewer than `min_tcs_per_feature` TCs (not in the baseline) fails the platform gate.

**Rationale:**

- **Bundle quality.** The implementer's context bundle is only as informative as the TCs in it. Four TCs frame the problem; one TC silos it.
- **Verification trust.** A green `product verify FT-XXX` only carries weight when the TC set is non-trivial. This rule makes that trust mechanical.
- **LLM accountability.** Agents tend to satisfy the assertions they see. More assertions = better behavioural fence. Less drift between "what the agent built" and "what the feature should do."
- **Bounded backfill.** The baseline mechanism converts a 92-feature problem into a tractable burndown that can be driven by the same orchestrator (via a TC-author worker) the rule supports.
- **Configurable, not hardcoded.** Per ADR-068, operator-tunable thresholds live in `.dec/config.toml` with a documented built-in default. A hardcoded "4" in the script would silently lock teams into one choice; a config key makes the choice reviewable, overridable per-environment via `DEC_*` env vars (useful for CI), and stays consistent with how every other knob in the CLI is exposed.

**Rejected alternatives:**

- **No minimum (status quo).** Already shown to underdeliver — 77% of completed features have fewer than four TCs, and 45% have exactly one. The rule exists because the floor is too low.
- **Hardcoded threshold of 4.** Locks every team into one number forever. Inconsistent with ADR-068's pattern for every other operator-tunable in the CLI. Rejected.
- **Variable minimum per feature complexity.** Requires judgement at authoring time and an enforcement model that quantifies "complexity." Adds discretion without adding mechanical signal. Rejected.
- **Higher minimum default (six, eight).** Risks pressure to file padding TCs across the whole repo. Four is the smallest forcing function that requires multi-axis thinking; teams that want stricter override via config.
- **Coverage axes as enum in TC frontmatter.** Would require a schema change and per-TC tagging. The recommended axes are documented narratively in this ADR; mechanical enforcement is deferred until the count rule is in steady state.
- **Hard-fail immediately, no baseline.** Blocks every PR until 92 features are backfilled. Operationally unrealistic; conflicts with phase 4 active work. Rejected in favour of the baseline ramp.
- **Grandfather all pre-existing features forever.** Preserves the failure mode the rule is designed to eliminate. The baseline is a snapshot, not a permanent exemption — it is depleted, not maintained.
