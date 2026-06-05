---
id: FT-163
title: 'decision-cli: cluster_dispatch framing window expanded to fit full feature_spec body'
phase: 4
status: complete
depends-on:
- FT-139
adrs:
- ADR-080
tests:
- TC-395
- TC-396
- TC-397
- TC-398
domains:
- api
domains-acknowledged: {}
---

## Description

Witnessed by the first non-prototype dispatch of [FT-141](FT-141)'s `add-artifact-type` cluster: [FT-147](FT-147) (the Archetype substrate) dispatched cleanly through the cluster but produced a **fictional struct shape** because the cluster's per-cell framing truncates the feature spec at **2000 chars**, which cuts off before the §Outputs section where the prescriptive `pub struct Archetype { ... }` block lives. The worker drafted a generic CRUD-shape from the §Description alone (id/name/description/contracts/audits) instead of the spec's intended `NamedNode`-based ontology shape with status enum, instance bindings, provenance, evidence.

The 2k cap was reasonable when the cluster prototype shipped — only `add-judge-worker` (a small TaskType) was witnessed and the §Description prose carried enough information. Once the cluster is asked to ship features whose §Outputs section prescribes Rust struct fields verbatim, 2k chars becomes architecturally insufficient.

Fix is one line in `cluster_dispatch::load_feature_framing` plus its TCs. The cluster's per-cell bundles grow from ~2k → up to ~50k chars of framing, but the per-cell context-window budget on qwen3-coder (256k tokens) absorbs this comfortably and the upstream-cell outputs (the bigger contributor by far) are unchanged. Marginal cost ≈ +€0.05 per cluster dispatch on Scaleway (≈ +250k input tokens / cluster across 5 LLM cells).

## Functional Specification

### Inputs

- `crates/decision-cli/src/features/drive/cluster_dispatch.rs::load_feature_framing` (current 2k-char cap).
- `crates/decision-cli/src/features/drive/cluster_dispatch.rs::truncate_for_framing` (the truncation helper).
- The witnessed FT-147 dispatch (€0.0317 + 270s) where the worker invented `Contract`, `AuditScope` types not in the spec.

### Outputs

**One constant change** in `cluster_dispatch.rs`:

```rust
// before
Ok(truncate_for_framing(&raw, 2000))

// after
Ok(truncate_for_framing(&raw, MAX_FRAMING_CHARS))
```

Where `MAX_FRAMING_CHARS: usize = 50_000` is defined as a module-level constant with a docstring citing this feature and the witnessed FT-147 dispatch.

**Truncation behaviour unchanged** when a spec exceeds the cap — same `\n…\n[spec truncated for cell framing]\n` suffix, same chars-not-bytes counting.

**No new constants beyond the cap.** No per-feature override, no `--full-spec` flag — the cap is large enough for every feature_spec in the catalog (longest current: FT-160 at ~12k chars) with 4× headroom.

### State

- **Modified on-disk:** `crates/decision-cli/src/features/drive/cluster_dispatch.rs` — one constant, one line in the helper call, one docstring.
- **No new files, no schema change, no orchestration-store mutation.**

### Behaviour

1. **`load_feature_framing` reads the full spec file as today.** Truncation happens at 50k chars now instead of 2k.
2. **Specs under 50k pass through unchanged.** Every current feature_spec lands intact.
3. **Specs over 50k truncate with the same suffix.** Pathological specs (hypothetical) get the same warning the old cap produced; cells get partial framing.
4. **No cluster behaviour change for existing dispatches** — FT-145 (`add-cli-subcommand`) and `add-judge-worker` clusters already produced correct output because the cells were small and matched existing patterns; their framing just gets richer now.

### Invariants

- **The truncation function stays char-based** (not byte-based). UTF-8 graphemes don't split.
- **The cap is a module-level `const`, not a field on a struct.** Operators don't tune it per dispatch — that would be an architectural change deferred.
- **No regression on small specs.** Specs ≤ 2000 chars produce identical framing before and after.
- **No regression on cluster audit semantics.** Same audit script invocations, same fail-fast behaviour — only the upstream framing got richer.

### Error handling

- **Spec file not found** → existing `cluster_dispatch: feature spec not found via pattern` error, unchanged.
- **Spec file read error** → existing IO error path, unchanged.
- **Spec over 50k chars** → truncated with the same warning suffix, same as today's over-2k path.

### Boundaries

- **In scope.** Constant bump + module docstring + 4 TCs.
- **Out of scope.** Per-feature framing config (`[cluster_framing] full_spec = true` in `task-types.toml`) — possible future enhancement once a spec exceeds 50k. Section-aware smart truncation (e.g., "always include §Functional Specification") — over-engineered for the witnessed need. Exemplar injection (sending `feedback.rs` source into the cell bundle as a prior-art example) — substantially larger architectural change, deferred. Per-cell context budget tuning — out of v1.

## Out of scope

- Per-feature / per-task-type framing config.
- Section-aware truncation.
- Exemplar injection into per-cell bundles.
- Per-cell context budget tuning.
- Streaming the spec to the worker in chunks.
