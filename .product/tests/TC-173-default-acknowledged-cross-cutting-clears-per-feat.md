---
id: TC-173
title: default-acknowledged-cross-cutting clears per-feature preflight gaps without requiring per-feature adrs link
type: exit-criteria
status: passing
validates:
  features:
  - FT-104
  adrs: []
phase: 1
runner: bash
runner-args: cd /home/hafeok/projects/product-cli && python3 -m pytest tests/test_default_acknowledge.py -v
runner-timeout: 120
last-run: 2026-05-28T07:41:47.795570592+00:00
last-run-duration: 0.7s
---

## Claim

When an ADR is listed in `[features] default-acknowledged-cross-cutting` in `product.toml`, `product preflight FT-XXX` does **not** report it as a cross-cutting gap on any feature that has not explicitly linked it. The feature's `adrs:` list remains unchanged; the acknowledgment is virtual, sourced from the config.

## Scenarios

### Setup

- A temp product-cli repo seeded via `product init`.
- One cross-cutting ADR `ADR-CC` (`scope: cross-cutting`).
- Two features:
  - `FT-LINKED` whose `adrs:` list contains `ADR-CC`.
  - `FT-UNLINKED` whose `adrs:` list does not.

### Scenario A — baseline without default-acknowledge

With `product.toml` containing no `default-acknowledged-cross-cutting` key:

- `product preflight FT-LINKED` → no gap for ADR-CC (it's linked).
- `product preflight FT-UNLINKED` → gap for ADR-CC (severity `missing`).

### Scenario B — default-acknowledge clears the gap

Add to `product.toml`:

```toml
[features]
default-acknowledged-cross-cutting = ["ADR-CC"]
```

Re-run preflight:

- `product preflight FT-LINKED` → no gap for ADR-CC; the explicit link wins (no special annotation needed; behaves as today).
- `product preflight FT-UNLINKED` → **no gap** for ADR-CC. The preflight output annotates `ADR-CC (default-acknowledged)` in the satisfied-coverage section so the operator sees *why* it's clean.

### Scenario C — feature frontmatter is untouched

After Scenario B, `cat .product/features/FT-UNLINKED-*.md` shows the `adrs:` frontmatter list is **unchanged** (still does not contain ADR-CC). The acknowledgment lives in `product.toml`, not in the feature.

### Scenario D — removing the entry restores the gap

Remove `ADR-CC` from `default-acknowledged-cross-cutting` in `product.toml`. Re-run preflight on `FT-UNLINKED`. The gap returns with severity `missing`. Confirms the default is config-driven and reversible.

### Scenario E — empty list behaves as absent

Setting `default-acknowledged-cross-cutting = []` (empty list) produces identical behavior to omitting the key entirely. Both preflight runs show the same gap for `FT-UNLINKED`.

## Runner

`pytest tests/test_default_acknowledge.py` (product-cli's test layout). The test uses a temp `.product/` fixture with the seeded ADR and features, mutates `config.toml` between scenarios, and asserts preflight output via JSON-format parsing rather than text scraping.

## Non-goals

- The `adrs-rejected:` opt-out path (TC-174 covers that).
- Drift detection between the config and the ADR catalog (TC-175 covers that).
- Per-domain or per-feature-class overrides — explicitly out of scope per FT-104.
- LLM-driven population of the default list (out of scope; v1 is operator-managed).