---
id: FT-103
title: 'decision-cli: Backfill cross-cutting ADR links across existing features'
phase: 3
status: complete
depends-on: []
adrs:
- ADR-066
tests:
- TC-172
domains: []
domains-acknowledged: {}
---

## Description

A one-time cleanup against the **"32 cross-cutting gaps on every feature"** smell surfaced when running `product preflight` on FT-101/FT-102. The slice does two coupled things:

1. **Re-scope mis-tagged ADRs** — demote ADRs whose true scope is feature-specific or domain rather than cross-cutting. This is the dominant lever and what actually reduces per-feature preflight noise.
2. **Backfill missing `feature ↔ ADR` links** on the ADRs that remain genuinely cross-cutting (and the domain-scoped ones whose domain matches existing features), so the system-wide assertion TC-172 (every accepted cross-cutting ADR has ≥1 implementer) passes.

**Diagnosis (confirmed via product-cli source-read).** `product preflight FT-XXX` iterates every ADR with `scope: cross-cutting` and asks per-feature: *"does this feature link the ADR, or share a domain with it?"* If neither, it's a gap. Decision-cli's `.product/` carried `scope: cross-cutting` on **32 of 66 ADRs (~48%)** — far past the design intent. Many were single-feature ADRs (e.g. `ADR-055` WorkerImage → only FT-086), several were domain-scoped (verification, feedback, worker-substrate), and a handful were forward-looking or cross-repo (ADR-065 Dagger deferred, ADR-044 Brief in product-cli). The check was *working as intended*; the catalog was misusing the tag.

This slice **does not** introduce code; it edits `.product/` frontmatter via `product adr scope` and `product feature link`. It is metadata-only.

A separate concern that this slice surfaces but does **not** solve: ~6-7 ADRs (ADR-001, ADR-004, ADR-012, ADR-013, ADR-014, ADR-021, ADR-041) are *system-wide invariants* satisfied by the platform itself (compile-time SDP for ADR-001, the GraphWriter chokepoint for ADR-041, FT-014 enforcement for ADR-013, fitness TCs for ADR-014/ADR-021). The current scope vocabulary (`cross-cutting` / `domain` / `feature-specific`) has no value that means "satisfied by a platform check, no per-feature link required". The user is adding a new `platform` (or `invariant` / `fitness`) scope value to product-cli's `AdrScope` enum as a separate change. Once that ships, the ~6 platform-pending ADRs migrate from `cross-cutting` to `platform` and the residual per-feature gap count drops to the ~5 truly cross-cutting decisions (ADR-002, ADR-005, ADR-016, ADR-017, ADR-038).

One subcommand → one slice — no subcommand. The slice is the re-scope pass + the link backfill + a TC asserting the resulting state.

## Functional Specification

### Inputs

- The current `.product/` state (66 ADRs, 103 features at slice-execution time).
- The criterion the re-scope applies: *"would a brand-new, unrelated feature plausibly need to consider this ADR? If yes, cross-cutting. If only features in a specific slice/domain need to, demote to `domain`. If only one feature owns it, demote to `feature-specific`."*

### Outputs

- 20+ ADRs re-scoped from `cross-cutting` to `domain` or `feature-specific`.
- 30+ missing `feature ↔ ADR` links backfilled via `product feature link`.
- One TC (TC-172) asserting post-state correctness.
- No source-tree changes.

### State

- Reads: every `.product/adrs/*.md` and `.product/features/*.md`.
- Writes: ADR frontmatter (`scope`, `domains`) and feature frontmatter (`adrs:`). All mediated by the product-cli tooling, which preserves frontmatter and reciprocates link edges.

### Behaviour

#### Re-scope mapping (the executed change)

| ADR | Before | After | Rationale |
|---|---|---|---|
| ADR-018 (VerificationVerdict schema) | cross-cutting | domain (data-model) | Only relevant to verification features. |
| ADR-021 (agreement metric) | cross-cutting | feature-specific | Implemented by FT-024; a fitness metric, not per-feature. |
| ADR-022 (feedback as flow class) | cross-cutting | domain (data-model) | Spans feedback slice FT-026..FT-033, not the whole repo. |
| ADR-023 (feedback vocabulary) | cross-cutting | domain (data-model) | Feedback domain. |
| ADR-024 (feedback lifecycle) | cross-cutting | domain (data-model) | Feedback domain. |
| ADR-025 (blocking vs non-blocking feedback) | cross-cutting | domain (api, data-model) | Feedback domain. |
| ADR-027 (authority declarations) | cross-cutting | feature-specific | Implemented by FT-030 only. |
| ADR-033 (capability-based model routing) | cross-cutting | domain (api, data-model, observability) | Worker-substrate domain. |
| ADR-034 (tiered escalation policy) | cross-cutting | feature-specific | Implemented by FT-062 only. |
| ADR-035 (bundle stakes) | cross-cutting | feature-specific | FT-056 + FT-063; narrow. |
| ADR-036 (Capability + RoleBinding catalog) | cross-cutting | domain (data-model, storage) | Catalog domain. |
| ADR-037 (Scaleway endpoint) | cross-cutting | domain (api, networking, security) | Worker-substrate domain. |
| ADR-039 (motivational predicates) | cross-cutting | feature-specific | FT-070's vocab. |
| ADR-040 (BoundaryArtifact) | cross-cutting | feature-specific | FT-071. |
| ADR-043 (full-chain traversal QueryTemplate) | cross-cutting | feature-specific | FT-075. |
| ADR-044 (Brief artifact in product-cli) | cross-cutting | feature-specific | Implementation lives in product-cli; cross-stream concern. |
| ADR-047 (capability-tag binding) | cross-cutting | domain (api) | Worker-substrate domain. |
| ADR-054 (LiteLLM worker SDK substrate) | cross-cutting | feature-specific | FT-081. |
| ADR-055 (WorkerImage artifact type) | cross-cutting | feature-specific | FT-086. |
| ADR-064 (LiteLLM proxy substrate) | cross-cutting | feature-specific | FT-096. |
| ADR-065 (Dagger deferred) | cross-cutting | feature-specific | Non-decision; closest fit until a `deferred` scope exists. |

#### ADRs that stay cross-cutting (and the platform-pending subset)

**Truly cross-cutting** (every new feature plausibly considers them):
- ADR-002 (graph-as-state)
- ADR-005 (value stream as graph-resident scope)
- ADR-016 (vertical-slice + SDP)
- ADR-017 (action-interpretation pairing)
- ADR-038 (dual provenance)

**Platform-pending** (satisfied by a platform check, will migrate to `platform` scope once product-cli ships it):
- ADR-001 (oxi-events as separate crate — compile-time SDP)
- ADR-004 (PROV-O for events — substrate)
- ADR-012 (per-stream working directories — substrate)
- ADR-013 (Code Structure and Quality Standards — enforced by FT-014)
- ADR-014 (fitness functions — meta-ADR)
- ADR-041 (SHACL at GraphWriter chokepoint — enforced by FT-001 + FT-073)

**Total remaining cross-cutting: ~11** (down from 32). Once the platform scope migration happens, **~5 truly cross-cutting** remain.

#### Measured impact (post-execution sample)

| Feature | Pre-rescope cross-cutting gap count | Post-rescope |
|---|---|---|
| FT-101 | 30 | 9 |
| FT-102 | 29 | 10 |
| FT-097 | 32 | 11 |

The drop matches the prediction: the new gap count equals the residual cross-cutting + platform-pending set, modulo whichever subset of those each feature has already linked.

#### Link backfill (the smaller lever)

For the ADRs that remain cross-cutting, FT-103 also runs `product feature link` between each ADR and its already-existing implementing features (e.g. ADR-001 → FT-001..FT-005, ADR-004 → FT-001/FT-009/FT-021/FT-069). This satisfies the system-wide assertion TC-172 (every cross-cutting ADR has ≥1 implementer) but does **not** further reduce per-feature gap counts — the preflight check is per-feature, not system-wide.

### Invariants

- **Idempotent.** Re-running the slice's calls produces no further changes.
- **Conservative re-scope.** Each demotion is justified in the mapping table above. When in doubt between `domain` and `feature-specific`, the slice chose `domain` (over-tagging is recoverable; under-tagging hides real concerns).
- **No code change.** This slice modifies only `.product/`.
- **TC-172 is the resulting fitness function.** Post-slice, every accepted ADR with `scope: cross-cutting` must have ≥1 feature in its `features:` list, with the exclusion + delegation list captured in the TC body.
- **Platform-pending ADRs are documented inline.** Once product-cli ships the `platform` scope value, a one-line follow-up slice (or this slice's amendment) migrates the listed six ADRs. The migration is mechanical: `product adr scope ADR-NNN --scope platform` per ID.

### Error handling

- `product adr scope` failing on an unknown scope value (e.g. attempting `platform` before product-cli ships it) — expected and gating; the slice does not attempt the platform migration until the scope value is available upstream.
- `product feature link` failing with E022 (unknown ID) — typo in the mapping table; fix and retry.
- No write-side errors observed during execution.

### Boundaries

- **In scope.** The 21 `product adr scope` calls per the mapping table; the ~30 `product feature link` calls (already executed); domain-tag additions on demoted-to-`domain` ADRs that lacked tags; the residual cross-cutting + platform-pending classification table; TC-172 backed by the post-slice state.
- **Out of scope.** Adding the `platform` scope value to product-cli (a separate, upstream product-cli change). Migrating the six platform-pending ADRs (depends on the upstream change). Auditing whether each demoted ADR's `feature-specific` or `domain` assignment is *correct* — the slice errs on the side of demotion; granular re-assignment can happen feature-by-feature later. Per-feature domain tagging (features could declare `domains:` to clear remaining `domain`-scope gaps, but that's a separate cleanup; this slice handles the ADR side only).

## Out of scope

- product-cli scope vocabulary extension (the `platform` value).
- Platform-pending ADR migration.
- Per-feature domain tagging.
- Code changes.
- Cross-stream link backfill (Brief etc. living in product-cli's graph).
- Correctness audit of each linked feature's implementation.
- A second pass on demoted ADRs to validate `feature-specific` vs. `domain` choice — the slice's choices are recoverable.
