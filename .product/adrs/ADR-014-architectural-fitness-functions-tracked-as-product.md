---
id: ADR-014
title: Architectural Fitness Functions Tracked as product-cli Artifacts
status: accepted
features: []
supersedes: []
superseded-by: []
domains:
- observability
scope: platform
content-hash: sha256:5367f3f82371a892630b0d483e2295bb82e8a5e4cc331be901e4a5ecc46d9e2f
source-files:
- CLAUDE.md
- scripts/checks/cross-cutting-rules-have-checks.sh
---

**Status:** Proposed

**Context:** The natural place to write down a code-quality rule is a CONTRIBUTING.md bullet, a README admonition, or a tribal-knowledge note in a Slack channel. None of these survive contact with LLM-driven implementation. An agent dispatched on FT-014 receives a context bundle of feature + ADRs + tests; tribal knowledge is not in the bundle, so it does not exist from the agent's point of view.

decision-cli already has an authoritative knowledge graph for engineering rules: its own product-cli setup at `.product/`. Features, ADRs, and test criteria live there with bidirectional links, schema validation, drift detection, and a context-assembly mechanism that produces deterministic bundles. The same machinery that delivers ADR-005 ("Value stream as a graph-resident scope") to the implementer is the right place to deliver "no source file exceeds 400 lines."

This ADR formalises the convention that decision-cli's *internal* product-cli graph — the one rooted at `.product/` in this repository — is the **single source of truth** for code-quality rules and other architectural fitness functions. Rules live as ADRs with `scope: cross-cutting`. Rule checks live as TCs with `runner: bash` (or `pytest`) pointing at scripts in `scripts/checks/`. Both are reachable to every implementation session through ordinary `product context` and `product verify --platform` calls.

This is distinct from the *external* product-cli project ([github.com/Hafeok/product-cli](https://github.com/Hafeok/product-cli)) which is a sibling repository decision-cli consumes via subprocess and MCP (per ADR-009). The "internal product-cli graph" referred to in this ADR is decision-cli's own usage of the product-cli tool to manage its own engineering artifacts — at `.product/features/`, `.product/adrs/`, `.product/tests/`.

**Decision:** Architectural fitness functions — including but not limited to the code-quality rules in ADR-013 — are tracked as first-class artifacts in the internal product-cli graph rooted at `.product/`. The graph is the source of truth; CI is the enforcer; the bundle is the carrier.

The contract has three parts:

### 1. Rules are ADRs

A code-quality rule, or any other architectural fitness function, is authored as an ADR in `.product/adrs/`. The ADR carries:

- `scope: cross-cutting` — so it surfaces in every feature's context bundle (per product-cli's cross-cutting-ADRs-always-in-bundle convention).
- A `domains:` list naming the concerns it governs (e.g. `observability`, `error-handling`).
- Enforcement detail in the ADR body — the actual rule, the thresholds, the script paths, the rationale.

Examples in decision-cli today: ADR-013 (code structure and quality), ADR-008 (worker contract — a *behavioural* fitness function on workers), ADR-001 (SDP boundary on `oxi-events` — a *structural* fitness function on the crate graph).

### 2. Checks are TCs

The mechanical check for a rule is authored as a TC in `.product/tests/`. The TC carries:

- `runner: bash` (or `pytest`, `cargo-test`) — declares how the check executes.
- `runner-args` — the script path or test name.
- `validates.adrs: [ADR-XXX]` — links back to the rule.
- `validates.features: []` — empty for cross-cutting checks; the rule applies to every feature.

product-cli's `product verify --platform` runs every TC linked to a cross-cutting ADR. The set of cross-cutting ADRs is exactly the set of rules; the set of their TCs is exactly the set of mechanical checks. There is no separate registry.

### 3. Enforcement is automated through `product verify --platform`

A pull request CI step runs `product verify --platform`. The exit code is the gate:

- `0` — every cross-cutting TC passes. Merge.
- `1` — at least one cross-cutting TC fails. Block.
- `2` — warnings only (e.g. a file is in the 300–400 line warning zone). Allow merge, surface in the PR comment.

There is no second CI pipeline for "code quality" and no separate config for "fitness functions." Both surface as ordinary `product verify --platform` runs. New rules are added to the system by authoring an ADR + TCs through `product author` or `product request apply` — the same flow used for any other feature.

---

### Why the internal graph and not the external product-cli repo

decision-cli could, in principle, depend on a shared rules library imported from somewhere. We reject that for three reasons:

1. **Bundle locality.** A rule that governs decision-cli's implementation must appear in decision-cli's context bundles, not in a sibling repository's. Cross-repo context bundles are not on the road map for slice 1 (per `decision-cli-slice-1-bounds.md` §10).
2. **Version coupling.** A rule is part of the system it governs. Bumping the 400-line threshold should be a decision-cli ADR amendment, reviewable in the same PR that changes the threshold-dependent check.
3. **Provenance.** Every rule must trace back to a reviewed decision in this repository's history. An imported rule has no local audit trail.

product-cli (the external project) is a tool decision-cli consumes — like `rustc` or `pytest`. We do not store our rust toolchain or our test framework as rules; we store *our* rules in *our* graph.

### Why ADRs and not a `.product/rules/` directory

A separate rules directory would fork the artifact model. product-cli already has the concepts: ADRs for decisions, TCs for checks, features for capabilities. Inventing `Rule` as a fourth artifact type would mean a separate schema, separate validation, separate context-bundle integration. Reusing ADR + TC keeps the surface single.

### Why cross-cutting scope and not `feature-specific`

A rule is the inverse of a feature. A feature is "this should happen in one place." A rule is "this should not happen anywhere." `scope: cross-cutting` is the framework's existing carrier for the latter.

---

### Lifecycle

- **Add a rule.** `product author adr` → describe the rule → product-cli scaffolds the ADR with `scope: cross-cutting` and the relevant domains → author the enforcement script under `scripts/checks/` → author one or more TCs with `runner` config → link them to the ADR → `product request apply` → CI on the PR validates `product graph check` and `product verify --platform`.
- **Change a rule.** ADRs go through the existing accepted-ADR amend flow (`product_adr_amend`). The amendment is recorded with a reason and a previous-hash; the request log carries the audit trail.
- **Retire a rule.** `product_adr_status` → `superseded` (with a `by:` link to the replacement) or `abandoned`. Cross-cutting TCs lose their parent rule and surface in `product graph check` until they are deleted or relinked.

---

**Rationale:**

- **Discoverability.** Every implementation session in decision-cli pulls a context bundle. With rules in the bundle, the implementing agent reads them at the same time as the feature. There is no separate "did you remember to follow the style guide?" step.
- **Atomic governance.** A rule change and its enforcement-script change land in one request. Either both are applied or neither is. The request log records the reason.
- **Drift detection out of the box.** `product drift check` already detects spec-vs-implementation drift on ADRs with `source-files`. Naming the enforcement script as a `source-file` on a rule ADR means drift on the rule itself is detectable — exactly the property we want.
- **Single mental model.** Contributors learn product-cli once. They do not learn product-cli plus a fitness-functions DSL plus a separate CI rules file. There is no second system.

**Rejected alternatives:**

- **External rules library (npm-style).** Loses bundle locality, loses version coupling. Rejected.
- **A dedicated `Rule` artifact type.** Forks the artifact model. The existing ADR + TC pair already encodes "decision + check." Rejected.
- **CONTRIBUTING.md and a CI yaml file.** No graph linkage, no bundle inclusion, no drift detection, no provenance. The exact failure mode this decision-cli framework exists to fix. Rejected.
- **Inline the rules in each feature's ADR.** Forces every feature author to re-litigate the rule. Rejected — the whole point of `scope: cross-cutting` is that the rule is authored once and surfaces everywhere.
