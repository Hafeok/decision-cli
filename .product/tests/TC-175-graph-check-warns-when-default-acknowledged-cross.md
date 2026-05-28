---
id: TC-175
title: graph check warns when default-acknowledged-cross-cutting drifts from the live ADR catalog
type: scenario
status: passing
validates:
  features:
  - FT-104
  adrs: []
phase: 1
runner: bash
runner-args: cd /home/hafeok/projects/product-cli && python3 -m pytest tests/test_default_ack_drift.py -v
runner-timeout: 120
last-run: 2026-05-28T07:41:47.795570592+00:00
last-run-duration: 0.8s
---

## Claim

`product graph check` surfaces drift between `[features] default-acknowledged-cross-cutting` and the live ADR catalog as warnings, in three forms: a listed ADR that no longer exists, a listed ADR whose scope changed away from `cross-cutting`, and a feature's `adrs-rejected:` entry that references an ADR not in the default-acknowledge list.

## Scenarios

### Setup

- Temp product-cli repo seeded via `product init`.
- Three cross-cutting ADRs: `ADR-ALIVE`, `ADR-GONE`, `ADR-RESCOPED`.
- `product.toml`:
  ```toml
  [features]
  default-acknowledged-cross-cutting = ["ADR-ALIVE", "ADR-GONE", "ADR-RESCOPED"]
  ```
- One feature `FT-OPTOUT` with `adrs-rejected:` listing `ADR-ALIVE` (valid) and `ADR-STRAY` (invalid — not in the default-acknowledge list).

### Scenario A — listed ADR no longer exists

Delete `.product/adrs/ADR-GONE-*.md`. Run `product graph check`. Assertions:

- Exit code: 0 (warnings only).
- The output contains a `W` warning whose code matches `W0XX` (the new drift-warning code allocated for this slice) and whose detail names `ADR-GONE` and `default-acknowledged-cross-cutting`.
- The warning's `hint` field suggests removing `ADR-GONE` from `product.toml` or restoring the ADR file.

### Scenario B — listed ADR's scope changed away from cross-cutting

Restore `ADR-GONE`. Now run `product adr scope ADR-RESCOPED --scope feature-specific`. Run `product graph check`. Assertions:

- A new `W` warning naming `ADR-RESCOPED` and the changed scope, suggesting the operator remove it from `default-acknowledged-cross-cutting` (it's no longer cross-cutting, so default-acknowledging it is incoherent).

### Scenario C — feature rejects an ADR not in the default-acknowledge list

The feature `FT-OPTOUT` has `adrs-rejected: [ADR-ALIVE, ADR-STRAY]`. Run `product graph check`. Assertions:

- A `W` warning naming `FT-OPTOUT` and `ADR-STRAY`, message *"rejecting an ADR that is not default-acknowledged has no effect; either add ADR-STRAY to default-acknowledged-cross-cutting or remove the rejection."*
- The valid rejection of `ADR-ALIVE` does NOT produce a warning.

### Scenario D — three warnings co-exist without masking each other

With all three drift conditions present simultaneously, `product graph check` emits all three warnings, not just one. The output is sorted (by code, then by ID) for deterministic snapshot testing.

### Scenario E — fixing each warning clears it independently

Address each warning one at a time and re-run `graph check` between fixes:

1. Remove `ADR-GONE` from `default-acknowledged-cross-cutting` → only the `ADR-RESCOPED` and `ADR-STRAY` warnings remain.
2. Re-scope `ADR-RESCOPED` back to `cross-cutting` (or remove it from the config) → only `ADR-STRAY` remains.
3. Remove `ADR-STRAY` from `FT-OPTOUT.adrs-rejected:` → zero warnings.

This pins the contract: each warning is independent and each fix is local.

### Scenario F — exit code unchanged by drift

In every scenario above, `graph check`'s exit code is 0. Drift is informational, not blocking — this preserves the v1 contract that `graph check` warnings do not gate CI. (A separate slice could promote one of these to error severity if the team decides to enforce; out of scope here.)

## Runner

`pytest tests/test_default_ack_drift.py`. Same fixture and JSON-output-parsing pattern as TC-173 / TC-174.

## Non-goals

- Auto-repair of drift (out of slice; v1 is operator-driven).
- Promoting drift warnings to errors (out of slice; would gate CI).
- Drift detection on the `platform-satisfied-adrs` list (covered by the sibling product-cli feature for the `platform` scope, not here).
- Migration of existing repos with broken-state config (the warnings make the broken state visible; cleanup is operator-driven).