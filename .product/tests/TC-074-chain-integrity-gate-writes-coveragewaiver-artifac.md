---
id: TC-074
title: chain-integrity gate writes CoverageWaiver artifact and lets dispatch proceed
type: scenario
status: unimplemented
validates:
  features: []
  adrs: []
phase: 2
---

## Premise

Feature `FT-U` references TCs `[T1, T2]`. No graph references `T2`. The caller invokes `dec implement FT-U --waive-coverage "Doc-only feature; verification is review-based per ADR-NNN"` (length ≥ 16).

## Acceptance Criteria

- A new `CoverageWaiver` artifact is persisted at `.dec/verify/waivers/CW-NNN.ttl`:
  - `dec:waiverFor = FT-U`,
  - `dec:waiverReason = "Doc-only feature; verification is review-based per ADR-NNN"`,
  - `dec:uncoveredAtWaive = [T2]` (the snapshot),
  - `prov:wasAttributedTo` and `dcterms:created` populated.
- The waiver is registered in the orchestration store via the `StreamWriter` chokepoint (SHACL passes).
- The dispatch proceeds: the implementer worker is invoked, the session is opened, PROV-O `prov:used <CW-NNN>` records the waiver.
- Exit code matches the worker's exit code (0 on success).
- Running the same waiver text a second time mints a new `CW-NNN+1` (waivers are not deduplicated; each dispatch records its own waiver).

## Notes

Validates the escape hatch is **artifact-based**, not flag-based. The on-disk file is auditable; the PROV-O linkage makes "which dispatch used which waiver" a graph query, not log archaeology.
