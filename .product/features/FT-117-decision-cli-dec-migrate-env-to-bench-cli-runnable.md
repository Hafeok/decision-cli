---
id: FT-117
title: 'decision-cli: dec _migrate-env-to-bench CLI runnable against live orchestration store'
phase: 4
status: complete
depends-on:
- FT-112
adrs: []
tests:
- TC-246
- TC-247
- TC-248
domains:
- data-model
domains-acknowledged:
  data-model: Surfaces existing FT-112 migration logic as an operator CLI. The data-model-domain ADRs govern the SPARQL UPDATE discipline (atomic transaction, idempotent, no quad loss); no new ADR-level decision beyond what FT-112 already shipped.
---

## Description

FT-112's spec described a hidden CLI
`dec _migrate-env-to-bench` that walks the live orchestration
store, runs the SPARQL UPDATE rewriting every ENV IRI prefix
and predicate, and is idempotent so operators can safely
re-run. The implementation that shipped under FT-112 only
exposed the migration logic as a callable function exercised
by TC-210 against an isolated test store; **no CLI was
landed, and the live workdir's store was never migrated**.

Witnessed today: the live workdir has 5
`dec:VerificationEnvironment` instances with `/ns/env/`
subject IRIs and `dec:envType` / `dec:ranInEnvironment`
predicates — pre-rename state preserved verbatim. The
symptoms cascade:

- `dec verify bench list` → "no benches yet" (filter
  `?bench a dec:VerificationBench` matches zero of the legacy
  5).
- `dec verify bench new --id BNCH-002` → "duplicate id" (the
  duplicate-check uses a different shape than `bench list`
  and matches against the legacy half-state).
- `dec drive ship --bench BNCH-002` → "not found:
  VerificationBench 'BNCH-002'" at iteration 0, before any
  worker dispatches.

TC-210 passed because it composed a temp store with the right
fixture shape and migrated *that*. The "TC ran the
migration" boolean was true; the "live workdir's store got
migrated" boolean was false; nobody checked the gap.

FT-117 closes the gap: ship the CLI command FT-112 promised,
and add a TC that exercises the CLI against a workdir-shaped
fixture (not an in-memory test store) so the next-similar bug
gets caught at TC time.

## Functional Specification

### Inputs

CLI surface (hidden subcommand, debug/migration use):

- `dec _migrate-env-to-bench` — read the orchestration store
  at `<workdir>/.dec/store/orchestration.nq`, run the
  rewrite, write back.
- `dec _migrate-env-to-bench --workdir <path>` — override
  the workdir.
- `dec _migrate-env-to-bench --dry-run` — print the
  triples that would be rewritten, exit zero without
  writing.

### Outputs

- New CLI handler under
  `crates/decision-cli/src/cli/migrate_env_to_bench.rs` (or
  fold into the existing `dec migrate` subcommand tree).
- The handler invokes the existing migration logic from
  FT-112's `core` module against the live store.
- Console output naming what was rewritten:
  ```
  rewriting env→bench IRIs in .dec/store/orchestration.nq
    /ns/env/ → /ns/bench/                       5 subjects rewritten
    dec:VerificationEnvironment → dec:VerificationBench   5 classes rewritten
    dec:envType → dec:benchType                  5 predicates rewritten
    dec:ranInEnvironment → dec:ranOnBench       <N> predicates rewritten
  ✓ migration complete; idempotent on re-run
  ```
- On `--dry-run`: same counts, but a leading `[DRY-RUN]`
  marker and no write.

### State

Mutates `<workdir>/.dec/store/orchestration.nq` in place via
SPARQL UPDATE inside a StreamWriter transaction. The
write-back is atomic per existing store semantics.

### Behaviour

1. **Open the store.** Walk up from `--workdir` for the
   `.dec/store/orchestration.nq`. Open via the existing
   store-loading path.
2. **Apply the SPARQL UPDATE** that FT-112 §Outputs
   described, covering three rewrite classes:
   - Subject IRI prefix: `https://decision-cli.dev/ns/env/`
     → `https://decision-cli.dev/ns/bench/`.
   - Class assertion:
     `dec:VerificationEnvironment` → `dec:VerificationBench`.
   - Predicate IRIs: `dec:envType` → `dec:benchType`,
     `dec:ranInEnvironment` → `dec:ranOnBench`.
   The single transaction commits all three rewrites
   atomically.
3. **Idempotent on re-run.** Re-running on an already-
   migrated store rewrites zero quads (the patterns no
   longer match) and exits zero with a "rewrote 0
   quads" message.
4. **Dry-run mode.** Same SELECT-then-UPDATE pipeline, but
   the UPDATE phase is skipped. The console shows what
   would change.
5. **Cross-check post-migration.** The handler runs a final
   SPARQL query: `SELECT (COUNT(*) AS ?n) WHERE { ?s a
   dec:VerificationEnvironment }`. If the count is non-zero
   after a non-dry-run pass, the migration failed; the
   handler exits non-zero with the count and a diagnostic
   hint. (Defence in depth — the UPDATE should always cover
   every match, but the cross-check catches subtle bugs in
   the query template or transaction semantics.)

### Invariants

- The semantic content of the store is preserved: every quad
  retains identical subject identity (modulo the prefix
  rewrite), predicate intent, and object value. Only the
  textual IRI forms change.
- Idempotent: two invocations produce byte-identical store
  state.
- Atomic: either all rewrites land or none. No observable
  half-migrated state at quad-count granularity.
- The dry-run output and the actual write produce
  byte-identical rewrite reports (modulo the `[DRY-RUN]`
  marker), so operators can preview safely.
- The handler does NOT modify on-disk `.dec/verify/**/*.ttl`
  files. Those are FT-112 §Outputs's separate scope; if
  they're stale, run them through a separate rewrite pass.

### Error handling

- Workdir not found / has no `.dec/store/orchestration.nq`:
  exit non-zero with "no orchestration store at <path>" and
  the hint to run `dec init` first.
- SPARQL UPDATE returns a query-engine error: roll back the
  transaction, exit non-zero with the engine's error text.
- Cross-check (step 5) reports residual
  `VerificationEnvironment` instances: exit non-zero with
  the count + sample IRIs. Indicates a UPDATE-pattern bug
  that needs investigation.

### Boundaries

- This feature does NOT touch on-disk `.ttl` files. If the
  workdir has `.dec/verify/env/ENV-NNN.ttl` lying around
  (un-renamed by the filesystem pass), that's a separate
  cleanup; this CLI only fixes the store. (A future
  `--also-rewrite-disk-ttl` flag could cover both in one
  pass, but isn't shipped here.)
- This feature does NOT change FT-112's existing core
  migration logic. It only exposes that logic as an
  operator-callable CLI and adds the cross-check.
- This feature does NOT introduce SPARQL UPDATE access
  through the `_sparql` debug command. The migration
  remains a fixed, named operation; arbitrary UPDATE
  through `_sparql` would need its own ADR.

## Out of scope

- **Migrating other vocabulary renames** beyond the FT-112
  env→bench scope. A future ENV-NNN-style rename gets its
  own CLI command on the same pattern.
- **Bulk multi-workdir migration.** Operators iterate
  workdirs manually if needed.
- **A backup / restore step before migration.** The
  store-side transaction is atomic; operators who want a
  belt-and-braces backup take a `cp` of the `.nq` file
  before invocation. Not a feature responsibility.
- **Auto-running the migration at `dec init` time.** The
  migration is for stores that *predate* the rename; fresh
  init doesn't need it. Auto-running would be wasted
  effort 99% of the time.
- **Reverting the migration** (bench → env). One-way
  street; if you want to roll back FT-112, revert the
  vocab in code and re-deploy, not at the store layer.
