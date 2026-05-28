---
id: TC-174
title: adrs-rejected re-introduces the gap with severity intentional and carries the reason through to the preflight report
type: scenario
status: failing
validates:
  features:
  - FT-104
  adrs: []
phase: 1
runner: bash
runner-args: cd /home/hafeok/projects/product-cli && python3 -m pytest tests/test_adrs_rejected.py -v
runner-timeout: 120
last-run: 2026-05-28T08:49:09.264687901+00:00
last-run-duration: 0.9s
failure-message: "ERROR: file or directory not found: tests/test_adrs_rejected.py\n\n"
---

## Claim

A feature whose frontmatter declares `adrs-rejected:` for a default-acknowledged ADR causes `product preflight` to flag that ADR as a gap with `severity: intentional` and to carry the rejection reason through to the preflight report. The opt-out is auditable, distinct from "forgot to link", and verifiable in the CLI output and the JSON/MCP envelope.

## Scenarios

### Setup

- Temp product-cli repo seeded via `product init`.
- One cross-cutting ADR `ADR-CC`.
- `product.toml`:
  ```toml
  [features]
  default-acknowledged-cross-cutting = ["ADR-CC"]
  ```
- One feature `FT-OPTOUT` whose frontmatter declares:
  ```yaml
  adrs-rejected:
    - id: ADR-CC
      reason: "This feature uses an alternative pattern because <stated rationale>."
  ```

### Scenario A — rejection surfaces as a distinct gap kind

Run `product preflight FT-OPTOUT --format json`. Assertions:

- The output's `cross_cutting_gaps` list contains an entry for `ADR-CC`.
- That entry's `severity` field equals `"intentional"` (not `"missing"`).
- That entry's `reason` field equals the literal frontmatter reason string.
- The exit code is unchanged (preflight has historically been a warning-only verb; this slice doesn't change exit semantics).

### Scenario B — text format renders the rejection visibly

Run `product preflight FT-OPTOUT` (default text format). Assertions:

- Stdout contains a line matching pattern `ADR-CC.*INTENTIONAL.*<reason snippet>` so a human reader can distinguish it from a missing-link gap.
- The rejection appears in a separate visual section (e.g. "Rejected cross-cutting concerns:" rather than mixed with the missing set).

### Scenario C — empty reason is rejected at frontmatter parse time

Author a feature with:

```yaml
adrs-rejected:
  - id: ADR-CC
    reason: ""
```

Run `product feature show FT-BADOPTOUT`. Assertions:

- Exit code: 1.
- Stderr names the empty `reason` field and references the SHACL/Pydantic validation that requires non-empty.
- The feature does not surface in `product feature list` (or surfaces with a `W` warning indicating it's malformed).

### Scenario D — rejection without default-acknowledge is incoherent

Remove `ADR-CC` from `default-acknowledged-cross-cutting`. `FT-OPTOUT` still has `adrs-rejected: [ADR-CC]`. Run `product graph check`. Assertions:

- Exit code: 0 (warnings, not errors).
- Output contains a `W` warning naming `FT-OPTOUT` and `ADR-CC`: *"rejecting an ADR that is not default-acknowledged has no effect; remove the adrs-rejected entry or add ADR-CC to default-acknowledged-cross-cutting"*.
- Preflight on `FT-OPTOUT` still reports ADR-CC as a gap, now with `severity: missing` (the rejection is ignored because there's nothing to reject from).

### Scenario E — `product feature reject` verb wires the frontmatter

Run `product feature reject ADR-CC --feature FT-NEW --reason "Stated rationale here."`. Assertions:

- Exit code: 0.
- The feature's frontmatter gains an `adrs-rejected:` entry matching the input.
- The reason is preserved verbatim.
- Subsequent preflight runs show the rejection per Scenario A.
- Re-running the same command is idempotent: a second invocation with the same args either no-ops or updates the reason in place; either is acceptable, the test pins which.

## Runner

`pytest tests/test_adrs_rejected.py`. Same fixture pattern as TC-173. The test asserts on JSON preflight output for the rejection-rendering assertions and on direct frontmatter file reads for the persistence assertions.

## Non-goals

- The base default-acknowledge behavior (TC-173 covers that).
- The drift validators on `graph check` (TC-175 covers that).
- A separate "unreject" verb (operators can edit frontmatter directly; the `reject` verb's idempotency covers updates).
- Rendering the rejection set in a dashboard (out of slice).