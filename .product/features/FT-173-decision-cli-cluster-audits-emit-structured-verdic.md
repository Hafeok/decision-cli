---
id: FT-173
title: 'decision-cli: cluster audits emit structured verdicts; repair targeting and session records consume them'
phase: 5
status: planned
depends-on:
- FT-171
adrs:
- ADR-087
- ADR-080
- ADR-082
- ADR-084
- ADR-081
tests:
- TC-437
- TC-438
- TC-439
- TC-440
domains:
- data-model
- error-handling
- observability
domains-acknowledged:
  error-handling: Error semantics are fully specified in ADR-087 (unrunnable on unparseable v1 verdicts) and preserve the ADR-013 exit-code contract.
  data-model: New quads are confined to the existing cluster SessionRecord shape under the orchestration named graph (FT-146); no new artifact types, ontology terms reviewed in ADR-087.
  observability: 'Observability is the feature''s subject matter: per-check verdict quads and session show rendering are specified in the body and TC-440.'
  ADR-083: FT-173 changes the audit-to-harness wire contract only; it introduces no tech detail that binds at archetype, instance, or feature level.
---

## Description

Implements [ADR-087](ADR-087): the contract between a cluster coherence audit and the audit-repair loop becomes a typed document instead of parsed prose. Audit scripts write a `dec-audit-verdict/v1` JSON document (per-check name, status, detail, implicated cells/files); the harness consumes the verdict for repair targeting ([FT-171](FT-171)), builds per-cell retry context from the failed checks, and persists per-check quads on the cluster SessionRecord ([FT-146](FT-146)) so `dec session show` and `dec drive show` can explain a failed cluster from the graph alone.

The concept is adapted from the pipeline factory's typed `GateResult` (per-check gate evaluators whose output mechanically becomes the retry prompt), extended with cell implication because decision-cli's retry unit is a cell, not a whole step.

## Functional Specification

### Inputs

- `CoherenceAuditSpec` ([FT-139](FT-139)) extended with `verdict: v1 | legacy-text` (default `legacy-text` for existing TaskTypes).
- The audit subprocess environment: harness sets `DEC_AUDIT_VERDICT_PATH=<sandbox>/.dec-audit-verdict.json` for `v1` audits.
- The cells' declared `output_path`s ([FT-166](FT-166)/[FT-170](FT-170)) for the file→cell join.
- `scripts/checks/cluster-audit-add-artifact-type.py` migrates to `v1` as the witness audit.

### Outputs

- A verdict parser in the harness (`dec-audit-verdict/v1` schema: `audit`, `checks[].{name,status,detail,implicates.{cells,files}}`).
- Repair-targeting path in `cluster_dispatch.rs` that derives the implicated set from the verdict (direct cell names ∪ output_path owners), replacing text extraction for `v1` audits; empty `implicates` on a failed check degrades to all cells **and records the degradation**.
- Per-cell retry context assembled from only the failed checks implicating that cell.
- SessionRecord extension: per-check quads (name, status, truncated detail, implicated cells, degradation flag, `verdict-noncompliant` mark for legacy audits).
- `dec session show` renders the per-check audit narrative for cluster IRIs.

### State

- New quads under the cluster session IRI in the orchestration named graph; no new artifact types.
- The verdict file lives in the sandbox and is harness-read only; it is not promoted with cell outputs.

### Behaviour

1. Harness invokes the audit with `DEC_AUDIT_VERDICT_PATH` set when `verdict: v1`.
2. Exit 0 → pass (verdict optional, persisted if present). Exit 1 + parseable verdict → repair targeting from the verdict. Exit 1 + missing/unparseable verdict on a `v1` audit → `unrunnable`. Exit 2 → unrunnable (unchanged).
3. `legacy-text` audits keep the FT-171 text-extraction path; failed runs add `verdict-noncompliant` to the session record.
4. Repair rounds and retry caps are unchanged ([FT-171](FT-171): max 2 rounds, 2 per-cell retries).

### Invariants

- For `v1` audits, no code path parses audit stdout for targeting — the verdict is the only targeting source.
- A failed check with declared implication never widens re-dispatch beyond its declared/derived cells.
- The exit-code runner contract (0/1/2, [ADR-013](ADR-013)) is preserved.

### Error handling

- Unparseable verdict JSON, unknown schema id, or checks referencing unknown cells → audit treated as `unrunnable` with a diagnostic naming the defect; never a silent pass, never a guessy fallback.

### Boundaries

- Only the cluster coherence-audit path changes; product-cli TC runners and `product verify` are untouched.
- Prompt/audit-script authoring conventions for *future* audits (archetype, seam) inherit the contract but their audits are out of this slice.

## Out of scope

- Migrating the other five TaskType audits to `v1` (they migrate as next touched, per ADR-087 §6).
- A SARIF emitter or any IDE-facing rendering of verdicts.
- Schema versioning beyond `v1` (a `v2` negotiation lands with its first consumer).
