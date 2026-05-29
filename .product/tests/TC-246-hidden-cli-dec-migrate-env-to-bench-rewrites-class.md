---
id: TC-246
title: Hidden CLI dec _migrate-env-to-bench rewrites class assertion and predicate IRIs in live store
type: scenario
status: unimplemented
validates:
  features:
  - FT-117
  adrs: []
observes:
- graph
- exit-code
phase: 4
runner: bash
runner-args: tests/scripts/tc-246-migrate-cli-rewrites-store.sh
runner-timeout: 60
---

## Description

The bug TC-210 didn't catch: TC-210 exercises the migration
*function* against a fixture store. TC-246 exercises the
migration *CLI* against a workdir-shaped store so the operator
path is verified, not just the unit-test path.

## Acceptance Criteria

Bash test:

1. Compose a temp workdir with a `.dec/store/orchestration.nq`
   pre-populated with three quads:
   - `<.../env/ENV-001> a dec:VerificationEnvironment` in named
     graph `<.../graph/verify-env>`.
   - `<.../env/ENV-001> dec:envType "ephemeral-tempdir"` (same
     graph).
   - `<vgr> dec:ranInEnvironment <.../env/ENV-001>` in named
     graph `<.../graph/verify-result>`.
2. Run `dec _migrate-env-to-bench --workdir <temp>`. Assert
   exit code 0.
3. Run SPARQL `SELECT (COUNT(*) AS ?n) WHERE { ?s a
   dec:VerificationEnvironment }`. Assert `?n=0`.
4. Run SPARQL `SELECT ?s WHERE { ?s a dec:VerificationBench }`.
   Assert one result with subject `<.../bench/BNCH-001>`.
5. Run SPARQL `ASK WHERE { ?s dec:benchType "ephemeral-
   tempdir" }`. Assert true.
6. Run SPARQL `ASK WHERE { ?vgr dec:ranOnBench <.../bench/
   BNCH-001> }`. Assert true.

Each rewrite class — class assertion, instance prefix, and
both predicate IRIs — must be covered. TC-210's flaw was
treating "instance IRI rewrite" as the whole rewrite; this TC
demands every class explicitly.
