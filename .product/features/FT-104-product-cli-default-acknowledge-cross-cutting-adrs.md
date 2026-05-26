---
id: FT-104
title: 'product-cli: Default-acknowledge cross-cutting ADRs via product.toml, with explicit per-feature opt-out'
phase: 3
status: complete
depends-on: []
adrs:
- ADR-066
tests:
- TC-173
- TC-174
- TC-175
domains: []
domains-acknowledged: {}
---

## Description

A **cross-repo feature**: the implementation lives in [product-cli](https://github.com/Hafeok/product-cli), this spec lives in decision-cli because that is where the need surfaced ([FT-103](FT-103) cleanup; matches the FT-076 cross-repo precedent — that feature also tracks a product-cli change from this graph).

Adds two coupled mechanisms to product-cli's preflight surface:

1. **`[features] default-acknowledged-cross-cutting`** — a list of ADR IDs in `product.toml` that are inherited by every feature as a *virtual* link. Preflight stops flagging them as gaps per-feature without each feature listing them in `adrs:`. Mirrors the `platform-satisfied-adrs` mechanism (which product-cli is gaining as part of the `platform` scope work) but for *acknowledgable* concerns rather than *platform-enforced* ones.

2. **`adrs-rejected:`** frontmatter field on features — explicit per-feature opt-out for a default-acknowledged ADR. Carrying a reason string is mandatory; preflight re-flags any ADR listed here as a gap (visible signal that *this* feature deliberately doesn't follow the rule, with the rationale next to the rejection).

Together these implement the "default-allow, opt-out explicitly" stance for the *small set of cross-cutting concerns that survive the platform-scope migration*. In decision-cli's specific case, after the platform scope ships and absorbs the 6 platform-pending ADRs plus the 4 structurally-enforced "truly cross-cutting" ones (ADR-002, ADR-005, ADR-016, ADR-038), the residual cross-cutting bucket is approximately empty — so this repo's `product.toml` block will likely list **0 or 1 ADRs** (ADR-017 if it stays cross-cutting; nothing if it demotes to `domain`). The mechanism is still valuable for any product-cli-using repo whose architecture has genuine per-feature opt-in/opt-out concerns; decision-cli's near-empty case is the well-architected limit, not the typical one.

One subcommand → one slice — no subcommand. The slice extends `product.toml` parsing, extends feature frontmatter parsing, and extends the preflight algorithm in three small touches.

## Functional Specification

### Inputs

#### `product.toml` extension

```toml
[features]
# ... existing fields preserved ...

# Cross-cutting ADRs every feature in this repo acknowledges by default.
# Listed ADRs do NOT appear as preflight gaps per-feature.
# Per-feature opt-out is via the feature's `adrs-rejected:` frontmatter list.
default-acknowledged-cross-cutting = ["ADR-NNN", "ADR-MMM"]
```

Empty list (or absent key) preserves current behavior: every cross-cutting ADR shows as a per-feature gap unless explicitly linked.

#### Feature frontmatter extension

```yaml
---
id: FT-XXX
title: ...
# ... existing fields ...

# Per-feature opt-out from a default-acknowledged cross-cutting ADR.
# Each entry MUST carry a reason; preflight surfaces this as a visible
# gap so the choice is auditable.
adrs-rejected:
  - id: ADR-NNN
    reason: "This feature deliberately bypasses graph-as-state because <reason>."
---
```

Empty list (or absent key) is the default — no opt-outs.

### Outputs

- Preflight behavior on a feature whose `adrs:` does not list a `default-acknowledged-cross-cutting` ADR: **no gap** (was: gap).
- Preflight behavior on a feature whose `adrs-rejected:` lists a `default-acknowledged-cross-cutting` ADR: **gap with severity "intentional" and the reason carried through to the report** (so dashboards distinguish "forgot to acknowledge" from "deliberately rejects").
- A new validation in `product graph check`: every ID in `default-acknowledged-cross-cutting` must exist as an accepted ADR with `scope: cross-cutting`. Mismatches → `W` warning with detail.
- A new validation in `product graph check`: every ID in any feature's `adrs-rejected:` must appear in `product.toml`'s `default-acknowledged-cross-cutting`. Rejecting an ADR that isn't default-acknowledged is incoherent → `W` warning.

### State

- `product.toml` — one new optional key in `[features]`.
- Each `.product/features/FT-NNN-*.md` may grow an optional `adrs-rejected:` frontmatter field.
- No new files; no schema migration required for existing artifacts.

### Behaviour

#### Preflight algorithm change (the substantive edit)

Today (in `src/domains/preflight.rs`, per the user's source-read):

```text
for each ADR where scope=cross-cutting:
  for each feature:
    if feature.adrs contains ADR OR feature shares domain with ADR:
      ok
    else:
      gap
```

After this slice:

```text
for each ADR where scope=cross-cutting:
  for each feature:
    if feature.adrs contains ADR OR feature shares domain with ADR:
      ok
    elif ADR in config.default-acknowledged-cross-cutting:
      if feature.adrs-rejected contains ADR:
        gap (severity=intentional, reason=<from frontmatter>)
      else:
        ok (default-acknowledged)
    else:
      gap (severity=missing)
```

Three new behaviors fall out:
1. **Default-acknowledge clears gaps silently.** A feature that didn't link the ADR and doesn't reject it is just fine — the config-level default applies.
2. **Explicit rejection is auditable.** A feature that opts out shows up in preflight with `severity=intentional` plus the reason, so the operator sees "I rejected this on purpose" without it looking identical to "I forgot."
3. **Drift detection at `graph check`.** Stale entries in either the config or a feature's `adrs-rejected:` produce warnings, keeping the two in sync.

#### CLI surfacing

- `product preflight FT-XXX` output gains a "(default-acknowledged)" annotation next to ADRs satisfied by the config-level default, so the operator can see *why* a previously-flagged ADR is now clean.
- A new verb (or `--show-defaults` flag on `preflight`) lists the active `default-acknowledged-cross-cutting` set. Useful for auditing what the repo has agreed to assume.
- `product feature reject ADR-NNN --feature FT-XXX --reason "..."` writes the `adrs-rejected:` entry. Manual frontmatter editing also works, but a verb keeps the rationale shape consistent (reason field is mandatory).

#### Decision-cli's expected config block (forward-compatible draft)

Once both this feature and the `platform` scope feature land in product-cli, decision-cli's `.product/config.toml` will likely look something like:

```toml
[features]
# ... existing fields preserved ...

# After the FT-103 re-scope + the platform scope migration, almost
# nothing remains truly cross-cutting in decision-cli. Most former
# cross-cutting ADRs are now `feature-specific`, `domain`, or
# `platform`. The few that survive (if any) go here.
default-acknowledged-cross-cutting = []

# Reserved by FT-104; populated as cross-cutting concerns arise that
# every feature respects by default but should be opt-out-able.
```

The empty list is the correct steady state for a well-architected catalog — and it demonstrates that the mechanism scales gracefully to zero. A repo with a less mature architecture might list 3-5 entries; decision-cli's near-empty case is the limit.

### Invariants

- **The config-level default does not override an explicit link.** If a feature *does* list the ADR in `adrs:`, that takes precedence and is treated as ordinary linking — no annotation, no special path.
- **`adrs-rejected:` requires a reason.** Empty-string reasons are SHACL-rejected at frontmatter validation time (the `product feature reject` verb requires `--reason`).
- **The two new validations are warnings, not errors.** Drift between the config and the catalog is a smell, not a blocker — operators may temporarily have a renamed ADR or an in-flight scope change.
- **Backwards-compatible.** Repos without the `default-acknowledged-cross-cutting` key behave exactly as today. Features without the `adrs-rejected:` field behave exactly as today.
- **`adrs-rejected:` is only valid for default-acknowledged ADRs.** Rejecting an ADR that isn't auto-acknowledged is incoherent (you can't reject something you weren't acknowledging) — caught by the graph-check validator.

### Error handling

- Stale `default-acknowledged-cross-cutting` entry (ADR was deleted, demoted, or superseded) → `W` warning in `product graph check`, exit 0; preflight ignores the stale entry.
- `adrs-rejected:` entry for an ADR not in the config — `W` warning in `product graph check`; preflight treats the entry as a regular missing-link gap (the rejection has no effect because there was nothing to default-acknowledge).
- `adrs-rejected:` entry with empty `reason` — `E` error at `product feature show` / `preflight` parse time; the feature file is malformed.
- `product feature reject` against an unknown ADR or feature → `E022` (existing error code).

### Boundaries

- **In scope (product-cli implementation).** `product.toml` parser extension for the new key; feature-frontmatter parser extension for `adrs-rejected:`; preflight algorithm update; `graph check` drift validators; CLI surface annotations (`(default-acknowledged)` rendering, `--show-defaults` flag, `product feature reject` verb); MCP twins per ADR-029.
- **In scope (decision-cli adoption).** Once product-cli ships, this repo updates `.product/config.toml` with the empty (or near-empty) `default-acknowledged-cross-cutting` list. That adoption is a follow-up of this feature, captured as a one-line config edit.
- **Out of scope.** The `platform` scope value itself (a sibling product-cli change the user is handling separately; the two features are independent but composable). Auto-population of the config from heuristics about which ADRs are commonly skipped. Per-ADR override granularity (e.g. "default-acknowledge ADR-X only for features in domain Y" — needless complexity v1). Cross-repo synchronization of the config (each repo manages its own list). UI / dashboard rendering of the rejection set (out of slice; queryable via `product feature list --rejected ADR-NNN` is enough).

## Out of scope

- The `platform` scope value (sibling product-cli change).
- Heuristic auto-population.
- Per-domain or per-feature-class override granularity.
- Cross-repo config sharing.
- Web/dashboard rendering of rejections.
- Migration tooling (the mechanism is opt-in; existing repos work unchanged with an empty/absent key).
- Decision-cli's specific config-block adoption (a one-line follow-up once product-cli ships).
