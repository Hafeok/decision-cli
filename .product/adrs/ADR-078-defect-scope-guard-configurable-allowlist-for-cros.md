---
id: ADR-078
title: Defect-scope guard — configurable allowlist for cross-cutting paths
status: accepted
features:
- FT-137
supersedes: []
superseded-by: []
domains:
- api
scope: domain
content-hash: sha256:10094a33f519c03c5d5832dd1c042eb4daef6d864f83e65f20e3158253908fd4
---

## Context

The defect-scope guard in `crates/decision-cli/src/features/finalize/mod.rs` prevents the implementer from drifting outside the feature's prior commit scope when the bundle carries defect feedback. The guard's "always-allowed" predicate is hardcoded to two prefixes — `.product/` and `.dec/` — via `is_system_path`. Everything else must be in the feature's prior `[FT-XXX]` commit set.

Witnessed failure on [FT-136](FT-136): a cross-cutting workspace restructure that needs to edit `Cargo.toml`, `Cargo.lock`, `CLAUDE.md`, `CONTRIBUTING.md`, `.gitignore`, plus delete the `crates/product-cli/` and `crates/product-shim/` stub directories. The first drive iteration shipped the `product_cmd/mod.rs` rewrite (commit `212cc01`). The next iteration's Phase-1 attempt (Cargo wiring + crate deletions) was blocked because those paths are neither in the prior `[FT-136]` commit's file set nor in the narrow `.product/`/`.dec/` allowlist:

```
defect-scope violation: worker modified files outside the feature's prior commit history:
  ["Cargo.lock", "Cargo.toml", "crates/decision-cli/Cargo.toml",
   "crates/product-cli/Cargo.toml", "crates/product-shim/Cargo.toml", ...]
```

The guard's instinct is correct — workers shouldn't drift on a defect-fix run. But its predicate is too narrow: certain path categories are inherently project-wide rather than feature-scoped. Build manifests, repo-level docs, CI/packaging configs, and VCS metadata fall outside any single feature's scope by design. The hardcoded `.product/` + `.dec/` allowlist reflects the original FT-017/FT-108 model where those were the only project-wide categories that existed.

## Decision

**Extend the defect-scope guard's "always-allowed" predicate to four additional default categories, plus a project-level configuration override.**

### Default allowlist (hardcoded)

1. **Build manifests** — any `Cargo.toml` (workspace root, per-crate), `Cargo.lock`, `package.json`, `package-lock.json`, `pyproject.toml`, `uv.lock`, `pnpm-lock.yaml`, `yarn.lock`. Workspace structure changes ripple across features by design.
2. **Repo-level docs** (root only) — `CLAUDE.md`, `README.md`, `CONTRIBUTING.md`, `LICENSE`, `LICENSE.md`, `LICENSE.txt`, `CODE_OF_CONDUCT.md`, `CHANGELOG.md`. Process documentation, not feature implementation.
3. **CI / packaging configs** — anything under `.github/`, `.cargo/`, `dist-workspace.toml`, `rust-toolchain.toml`, `rust-toolchain`. Cross-cutting tooling.
4. **VCS metadata** — `.gitignore`, `.gitattributes`. Repo policy.
5. **(existing)** `.product/` and `.dec/` — artifact graph and orchestration store.

### Project-level override

A new `[scope-guard]` table in `.product/config.toml` lets each project declare additional always-allowed patterns:

```toml
[scope-guard]
always-allowed = [
    "scripts/checks/**",
    "deny.toml",
]
```

Patterns support `**` globs. Absent table → empty extras → defaults only.

### Boundaries

- The override is **additive**, not subtractive. The config can grow the allowlist but cannot remove a default. (If a default is wrong, the code is the right place to fix it; subtraction would silently weaken the guard.)
- Per-feature `scope-extra: [...]` frontmatter is **deliberately deferred** — adding per-feature complexity to a guard already prone to false positives would erode operator trust. Re-evaluate if project-level config proves insufficient.
- The guard's predicate still applies only to defect-scoped runs. Initial-implementation runs (no prior `[FT-XXX]` code commits) remain unrestricted, as today.

## Rejected alternatives

### Keep the narrow `.product/` + `.dec/` allowlist

Rejected — witnessed failure on FT-136 proves it's too narrow for cross-cutting work. The same shape would block every future workspace restructure, ADR-driven dependency change, or doc overhaul.

### Parse the feature_spec body for declared file paths

A spec's `State` section enumerates files explicitly ("Updated on-disk: root `Cargo.toml`, ..."). Extract that list per-feature and use it as the allowed set. Rejected — markdown-parser fragility, spec-body churn, and the more general "cross-cutting paths are project-wide regardless of any one feature" intuition is the cleaner abstraction.

### Disable the guard for cross-cutting features

A `scope: cross-cutting` frontmatter field could skip the guard entirely on those features. Rejected — the guard is most useful precisely *for* cross-cutting work, where worker drift is most damaging. A wider allowlist preserves the guard's protective intent while removing the false positives that block legitimate scope.

### Hand-massage the first commit to seed the allowed set

Make the operator commit `Cargo.toml` once under `[FT-XXX]` to put it in the per-feature allowed set. A workaround, not a fix. Rejected — every cross-cutting feature would need that ceremony; defeats the autonomous-shipping goal of `dec drive ship`.

### Per-feature `scope-extra: [paths]` in spec frontmatter

Let the feature spec declare specific extra paths it expects to touch. Rejected for this slice — see Boundaries above. Project-level is enough for now; per-feature can land later if the witnessed misfire pattern recurs at a per-feature granularity rather than a per-project one.

## Consequences

### Positive

- **Cross-cutting features unblock autonomously.** FT-136, FT-106, and similar workspace-restructure slices can defect-loop through `dec drive ship` without manual intervention.
- **Project policy in one place.** `[scope-guard]` in `.product/config.toml` lets each project declare its always-allowed set without code changes.
- **Defaults are deliberate and recorded.** The four hardcoded categories are common across Cargo/Rust + GitHub projects; the rationale is documented here.

### Negative / accepted trade-offs

- **Slightly larger drift surface.** A misbehaving implementer can now edit `Cargo.toml` or `CLAUDE.md` even on a re-fix iteration. Mitigated by the fact that these categories are PR-reviewed regardless; the guard was never the only line of defense.
- **Defaults assume a Cargo/Rust + GitHub project shape.** Other ecosystems (Python, Node, GitLab) get partial coverage. The config override is the escape hatch.
- **No per-feature granularity yet.** A feature that needs to touch an unusual path (say, a new top-level dir) must either add it to the project config or land its first commit manually to seed the allowed set. Acceptable cost vs. the per-feature complexity rejected above.

### Relationship to prior decisions

- **FT-017** (Implementer finalizes its run) introduced the finalize step; this ADR refines the guard portion of that step.
- **FT-108** (Feedback-aware code-writer dispatch) is what makes the defect-scoped path live; the wider allowlist makes that path workable for cross-cutting features.
- **FT-114** is the prior witnessed misfire that motivated the current "no prior non-system commits → bypass" branch in `finalize/mod.rs:146-159`. That bypass remains; this ADR addresses the orthogonal case where prior code commits *do* exist but the iteration legitimately needs to touch cross-cutting paths outside them.

## Status

Proposed. Promotes to accepted once the implementation slice ships and a regression-guard TC asserts the predicate behaviour on the four default categories plus the config override.
