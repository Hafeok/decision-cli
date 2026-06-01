---
id: TC-260
title: FT-120 orphan SPARQL query identifies orphaned defects
type: scenario
status: unimplemented
validates:
  features:
  - FT-120
  adrs:
  - ADR-024
phase: 4
runner: cargo-test
runner-args: features::ft_120_retract_orphan_defects::tests::tc_260_orphan_query_identifies_candidates
runner-timeout: 30
---

## Description

`features::ft_120_retract_orphan_defects::query::find_orphan_defects`
returns exactly the open feedback IRIs whose source VG no longer has
a step verifying their source TC.

## Acceptance criteria

1. **Empty store.** Returns `Ok(empty)`.
2. **Defect from VG that still covers the TC.** The graph has VG-001
   with a step that verifies TC-001; an open defect references
   (VG-001, TC-001). Query returns empty (not orphaned).
3. **Defect from VG that no longer covers the TC.** The graph has
   VG-001 with no step verifying TC-001 (TC migrated away); an open
   defect references (VG-001, TC-001). Query returns exactly that
   feedback IRI.
4. **Terminal-state defect.** A defect already in `closed`,
   `rejected`, or `superseded` state is not returned even when its
   source VG no longer covers the TC.
5. **Non-VGR-sourced defect.** A defect whose `dec:sourceSession`
   is not a VGR session (e.g. an implementer-emitted defect) is not
   returned, regardless of topology.

## Runner

`cargo-test` against the new module's `tests.rs`.
