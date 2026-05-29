---
id: FT-112
title: 'decision-cli: rename verification ENV-NNN to BNCH-NNN, free --env for deployment target'
phase: 4
status: complete
depends-on: []
adrs: []
tests:
- TC-208
- TC-209
- TC-210
- TC-211
- TC-212
- TC-213
- TC-214
domains:
- api
- data-model
domains-acknowledged:
  api: Rename touches the CLI surface (--env → --bench, verify env → verify bench) but introduces no new API contract; the api-domain ADRs (slice+adapter, etc.) constrain the call-site updates without independent decision.
  data-model: Renames IRI vocab and rewrites the orchestration store; preserves semantic content of every quad. The data-model-domain ADRs govern the migration discipline (single SPARQL UPDATE transaction, idempotent) without a new decision.
---

## Description

Two distinct concepts have been sharing the noun "environment"
in decision-cli's vocabulary, and the collision now blocks the
natural CLI surface for FT-111:

- **Deployment environment** (`local`, `dev`, `staging`, `prod`)
  — the conventional CI/CD reading. Where a verified feature
  ends up after `dec drive ship` succeeds. Today only `local`
  exists (commit to workdir = ship to operator's machine);
  multi-target deployment is future work.
- **Verification bench** (`ENV-001`, `ENV-002`, …) — the
  orchestration-store state a graph runs *against* during
  verify. Today's `ENV-NNN` lets graphs be exercised under
  varied state (clean slate, populated, mid-cycle, etc.).
  This is the concept the existing vocabulary already encodes.

The verification axis preempts the more intuitive operator
reading of "environment." Operators reading
`dec drive ship --env local` expect deployment semantics; they
get a verification-bench lookup that doesn't have a record for
`local` and fails.

This feature renames the verification axis from `Environment` /
`ENV-NNN` to `Bench` / `BNCH-NNN` across the entire codebase and
store, freeing the noun `environment` and the CLI flag `--env`
for the future deployment-target dimension. Deployment dispatch
is NOT implemented here — that's a downstream feature; this is
the vocabulary correction that makes deployment dispatch
implementable later without first un-doing a naming mistake.

Migration is mechanical: the `verify_env.rs` vocab module
defines a small fixed set of IRIs; renaming the constants and
their string values, then walking the orchestration store to
rewrite the prefix, completes the data side. Every Rust call
site, every CLI subcommand, every SPARQL query, every .ttl file
on disk, every feature/TC reference, every doc reference gets
updated in one pass.

## Functional Specification

### Inputs

Renames in source and on-disk artifacts. No new CLI inputs at
runtime; existing CLI surfaces change shape (see Outputs).

For migration tooling, one new bench-rewrite subcommand:

- `dec _migrate-env-to-bench` — hidden one-shot CLI that opens
  the workdir's orchestration store, runs the SPARQL UPDATE
  that rewrites every `ENV-NNN` IRI to `BNCH-NNN` and every
  `https://decision-cli.dev/ns/env/...` prefix to
  `https://decision-cli.dev/ns/bench/...`. Idempotent
  (re-running on an already-migrated store is a no-op).

### Outputs

**Vocabulary module** (`crates/decision-cli/src/core/vocab/`):

- `verify_env.rs` → renamed to `verify_bench.rs`.
- `IRI_DEC_VERIFICATION_ENVIRONMENT` → `IRI_DEC_VERIFICATION_BENCH`
  with value `https://decision-cli.dev/ns#VerificationBench`.
- `IRI_DEC_ENV_TYPE` → `IRI_DEC_BENCH_TYPE` with value
  `https://decision-cli.dev/ns#benchType`.
- `IRI_DEC_GRAPH_VERIFY_ENV` → `IRI_DEC_GRAPH_VERIFY_BENCH` with
  value `https://decision-cli.dev/ns/graph/verify-bench`.
- `IRI_DEC_ENV_PREFIX` → `IRI_DEC_BENCH_PREFIX` with value
  `https://decision-cli.dev/ns/bench/`.
- `IRI_DEC_RAN_IN_ENVIRONMENT` (in `verify_result.rs`) →
  `IRI_DEC_RAN_ON_BENCH` with value
  `https://decision-cli.dev/ns#ranOnBench`.

`IRI_DEC_LEDGER_ENVIRONMENT` (in `auto_dispatch.rs`) is NOT
renamed — it's a ledger-side concept (different axis) and
out of scope here; if it collides later, address separately.

**CLI surface**:

- `dec verify env` subcommand tree → renamed to `dec verify bench`
  (subcommands: `list`, `new`, `show` — same set, same body
  logic, only the verb changes).
- `dec drive ship --env <ENV-NNN>` → `dec drive ship --bench <BNCH-NNN>`.
- `dec verify graph generate --environment <ENV-NNN>` →
  `dec verify graph generate --bench <BNCH-NNN>`.
- `dec verify feature --env <ENV-NNN>` → `dec verify feature --bench <BNCH-NNN>`.

The flag `--env` becomes reserved (parser-level accept-but-error)
with a deprecation message pointing operators at `--bench`. After
two CLI versions, the parser stops accepting it; this feature
ships the reservation.

**On-disk artifacts**:

- Every `.ttl` file under `.dec/verify/` (graph, result,
  env→bench rename) gets its IRIs rewritten in place by the
  migration step. The directory `.dec/verify/env/` is renamed
  to `.dec/verify/bench/`.
- Every `.product/features/*.md` and `.product/tests/*.md`
  body reference to `ENV-002` (etc.) is rewritten to
  `BNCH-002`. Frontmatter fields that reference env IRIs are
  rewritten too.
- Bash test scripts under `tests/scripts/` that pass
  `--env ENV-002` get rewritten to `--bench BNCH-002`.

**Migration tool** — `dec _migrate-env-to-bench` — single
SPARQL UPDATE over the orchestration store:

```sparql
DELETE { GRAPH ?g { ?s ?p ?old_iri } ?s2 ?p2 ?old_iri . }
INSERT { GRAPH ?g { ?s ?p ?new_iri } ?s2 ?p2 ?new_iri . }
WHERE {
  { GRAPH ?g { ?s ?p ?old_iri } }
  UNION
  { ?s2 ?p2 ?old_iri . FILTER(!isBlank(?s2)) }
  FILTER(STRSTARTS(STR(?old_iri),
                   "https://decision-cli.dev/ns/env/"))
  BIND(IRI(CONCAT(
    "https://decision-cli.dev/ns/bench/",
    STRAFTER(STR(?old_iri), "https://decision-cli.dev/ns/env/")
  )) AS ?new_iri)
}
```

Plus a second UPDATE block for the predicate-IRI rewrites
(`envType` → `benchType`, `ranInEnvironment` → `ranOnBench`,
`VerificationEnvironment` → `VerificationBench`).

### State

No new persistent state. The migration rewrites existing
quads in the orchestration store; subsequent reads see the
new IRIs.

### Behaviour

1. **Source rename pass.** Find-and-replace every Rust
   identifier and string IRI in the codebase per the
   Outputs table. `core/vocab/verify_env.rs` →
   `verify_bench.rs`; rename the module re-export in
   `core/vocab/mod.rs`. Every call site adjusts its
   imports.
2. **CLI surface migration.** Rename the `verify env`
   subcommand to `verify bench` (CLI handler, clap struct,
   help text). Rename `--environment` / `--env`
   verification-bench flags to `--bench` on `verify graph
   generate`, `verify feature`, `drive ship`. Add a
   `--env` parser hook that accepts the flag but errors
   with `--env is reserved for future deployment-target use;
   pass --bench BNCH-NNN instead`.
3. **Documentation and product-graph rename.** Update
   every `.product/features/*.md` and `.product/tests/*.md`
   that mentions `ENV-NNN` in body or frontmatter; rewrite
   to `BNCH-NNN`. Update every `tests/scripts/*.sh` that
   invokes the CLI with `--env ENV-NNN`.
4. **On-disk .ttl rewrite.** Walk every `.ttl` under
   `.dec/verify/`; for each file, rewrite occurrences of
   the env IRI prefix and predicate IRIs in-place. Rename
   the `.dec/verify/env/` directory to `.dec/verify/bench/`.
5. **Store migration.** Invoke `dec _migrate-env-to-bench`
   to walk the orchestration store and apply the two SPARQL
   UPDATE blocks (instance prefix + predicate IRIs).
6. **Verify clean state.** After the migration, run
   `cargo test --workspace` (everything green), `dec verify
   feature FT-XXX --bench BNCH-002` (returns same verdict
   as before), `dec drive ship FT-XXX --bench BNCH-002`
   (runs identically), and one round of the FT-111 sweep
   to confirm no source or store reference to the old
   `ENV` vocabulary remains.

### Invariants

- The semantic content of the orchestration store is
  preserved: every quad's subject, predicate, object
  carries identical meaning, just with the IRI suffix /
  prefix rewritten. No quads are added or removed in
  meaning (only added or removed because their IRI form
  changes).
- The verify pipeline's verdict for any (feature, bench)
  pair is unchanged: same TCs, same step runners, same
  evidence, same VGRs, same aggregate verdicts. Only the
  IRI naming differs.
- The migration is idempotent: running
  `dec _migrate-env-to-bench` twice in a row produces no
  diff on the second invocation. Re-running on an
  already-migrated store is safe.
- `--env` is *parser-reserved* but not *meaningfully
  accepted* — a user invocation with `--env` errors out
  with the deprecation message and exits non-zero; no
  fallback to "treat as --bench".
- Backwards compatibility window: the `--env` reservation
  ships in this version; the next CLI minor version
  removes the reservation entirely (and `--env` becomes
  available for the future deployment-target feature). No
  silent acceptance during the window.

### Error handling

- A `.ttl` file that fails to parse during the on-disk
  rewrite pass is reported by path and skipped (with a
  warning); the migration continues. Partial migration is
  acceptable because the store update + Rust rename are
  the load-bearing parts; on-disk .ttl files get rewritten
  to keep them consistent with the store.
- The store migration runs inside a transaction; on
  partial failure the orchestration store rolls back to
  the pre-migration state and the operator gets a clear
  error.
- `--env ENV-NNN` after the rename: parser accepts the
  flag, prints the deprecation message, exits non-zero.
  Does not attempt to look up the ENV-NNN — the rename is
  complete, so the lookup would fail anyway.
- Migration tool invoked on a store with no ENV IRIs:
  no-op success.

### Boundaries

- This feature does NOT introduce deployment-target
  semantics. `--env` is *reserved*; no `local`, `dev`,
  `staging`, `prod` handling lands here. That's a future
  feature.
- This feature does NOT touch
  `IRI_DEC_LEDGER_ENVIRONMENT` in the auto-dispatch vocab;
  ledger-environment is a different axis.
- This feature does NOT change the on-disk file format of
  `.ttl` graphs or VGRs beyond the IRI rewrite. The Turtle
  syntax, the property names (except the renamed ones),
  the step structure — all unchanged.
- This feature does NOT modify the planner or driver
  logic (FT-110, FT-111). The cycle detector and the
  ship-all sweep both continue to work; they just read
  `BNCH-NNN` from CLI args instead of `ENV-NNN`.

## Out of scope

- **Deployment-target dispatch.** Implementing actual
  `dec drive ship --env dev` to push the verified feature
  to a remote dev cluster (or staging, or prod) is the
  reason for the rename, but it doesn't ship here.
  Separate feature(s) downstream.
- **A backwards-compatibility shim that accepts `--env`
  as a synonym for `--bench`.** Quietly accepting a wrong
  flag during a deprecation window confuses the operator's
  mental model. Hard reservation with a clear error is the
  better discipline.
- **Adopting the rename in downstream operator scripts the
  user runs outside this repo.** Out of project scope; the
  rename ships with deprecation guidance, downstream
  scripts adapt at their own pace.
- **Renaming `IRI_DEC_LEDGER_ENVIRONMENT`** (the
  auto-dispatch ledger axis). Different concept,
  different feature; address only if it collides later.
- **A two-phase migration tool that walks both the source
  tree and the store from one command.** The source
  changes are PR review territory; the store migration is
  CLI tooling. Bundling them into one black-box command
  hurts review-ability.
