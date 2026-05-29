---
id: TC-214
title: On-disk .ttl files under .dec/verify carry BNCH-prefix IRIs after migration
type: scenario
status: unimplemented
validates:
  features:
  - FT-112
  adrs: []
observes:
- file
- exit-code
phase: 4
runner: bash
runner-args: tests/scripts/tc-214-disk-ttl-rewritten.sh
runner-timeout: 60
---

## Description

The orchestration store is the source of truth for runtime
queries, but the `.dec/verify/**/*.ttl` files are the
authoritative source for graph definitions (per ADR-028). After
the migration, both surfaces must agree — leaving ENV IRIs on
disk while the store carries BNCH IRIs would cause the planner's
`graphs_exist_for_feature` (which reads disk) and the matcher
(which reads store) to disagree on which graphs are active.

## Acceptance Criteria

Bash test:

1. Seed `.dec/verify/graph/VG-001.ttl` and
   `.dec/verify/result/VGR-001.ttl` with IRIs that mention
   `<https://decision-cli.dev/ns/env/ENV-002>` and the predicate
   `dec:environment`.
2. Confirm `.dec/verify/env/` exists (with at least a stub
   `ENV-002.ttl`).
3. Run the migration: `dec _migrate-env-to-bench`. Assert exit 0.
4. Re-grep every `.ttl` under `.dec/verify/`:
   - Zero hits for `/ns/env/ENV-` (the old IRI prefix).
   - At least one hit for `/ns/bench/BNCH-` (the new IRI prefix).
   - Zero hits for the predicate `dec:environment` or
     `dec:ranInEnvironment` (renamed to `dec:bench` and
     `dec:ranOnBench` respectively).
5. Assert `.dec/verify/env/` no longer exists and
   `.dec/verify/bench/` exists, with the renamed file
   `BNCH-002.ttl` inside.
