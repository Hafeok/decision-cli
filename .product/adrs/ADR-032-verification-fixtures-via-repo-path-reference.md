---
id: ADR-032
title: Verification fixtures via repo-path reference
status: accepted
features:
- FT-053
supersedes: []
superseded-by: []
domains: []
scope: domain
content-hash: sha256:83faa9d8dbbb29305931b031401a56b37271c171bdafe57159c6867d16d2c862
---

## Context

[ADR-028](ADR-028) introduced `dec:VerificationEnvironment` as a typed execution context, with `dec:setup` and `dec:teardown` carrying free-form shell snippets. The seeded `ephemeral-cli` env demonstrates the smallest viable shape: `mkdir -p $DEC_VERIFY_TMP && cd $DEC_VERIFY_TMP`.

This works for verifying flows whose preconditions are nil — e.g. `dec init` itself, which is self-bootstrapping. It does **not** work for flows whose preconditions are rich:

- `dec implement FT-XXX` needs a seeded product-cli graph (feature_spec, ADRs, TCs in `.product/`), a `code-writer` worker on `$PATH` ([FT-013](FT-013)), and a target source tree.
- A future `dec drive <goal>` flow would need a full multi-role world to even dispatch.
- Any flow exercising real git operations needs a pre-initialised repo.

Encoding all of that inline in `dec:setup` is technically expressible but operationally bad: the snippet grows to dozens of lines of shell that aren't diffable as code, aren't versioned alongside the host code they shadow, and aren't independently authorable. Each new flow that needs preconditions would mint another mega-shell-snippet env.

The verification graphs authored on 2026-05-22 against ENV-002 (`dec init` flow — [VG-002](VG-002), [VG-003](VG-003), [VG-004](VG-004), [VG-005](VG-005)) sidestepped this — `dec init` has no external preconditions. The first dec-implement graph hits it immediately.

## Decision

Introduce a single optional predicate on `dec:VerificationEnvironment`: `dec:fixtureSource`, whose object is a string literal naming a repo-relative path. By convention, fixture trees live under `tests/fixtures/<name>/`.

```turtle
<env:dec-implement> a dec:VerificationEnvironment ;
    dec:envType         "ephemeral-tempdir" ;
    dec:safetyClass     "isolated" ;
    dec:allowedOps      ( "shell" "filesystem" "sparql-local" ) ;
    dec:fixtureSource   "tests/fixtures/dec-implement-basic" ;
    dec:setup           "mkdir -p \"$DEC_VERIFY_TMP\" && cd \"$DEC_VERIFY_TMP\"" ;
    dec:teardown        "rm -rf \"$DEC_VERIFY_TMP\"" .
```

### Runner contract

When the future graph executor materialises an env that carries `dec:fixtureSource`, it runs in this order:

1. Resolve the fixture path against the repo root (the working directory containing the `.dec/` it found via the [ADR-012](ADR-012) walk).
2. Execute `dec:setup` (creates the tempdir; conventionally `cd`s into it).
3. Recursively copy the fixture tree's contents into the env's CWD (`cp -a <fixture>/. <cwd>/`).
4. If `<cwd>/bin/` exists after the copy, prepend it to `$PATH` for the env's lifetime. This lets fixtures ship deterministic stubs (e.g. a `code-writer` that emits a fixed `CodeChange` JSON) that override host binaries.
5. Execute the graph's steps.
6. Execute `dec:teardown`.

The copy is `cp -a` (preserve mode/timestamps, recursive) and idempotent — re-running the same env/fixture pair against a clean tempdir is reproducible.

### Repo convention

- Fixture trees live under `<repo>/tests/fixtures/<name>/`.
- Each fixture is self-contained: any file or subdirectory it ships will exist verbatim in the materialised env.
- `bin/` subdirectories are the conventional location for deterministic stub binaries.
- A fixture is conceptually content-addressed by its tree hash, which git already tracks. Per-fixture documentation is optional (`README.md` in the fixture dir).

### Authoring surface

- `dec verify env new --fixture-source <path>` (and the matching MCP arg) accepts the optional value. Validation: non-empty string, repo-relative (no leading `/`, no `..` segments after normalisation), points at an existing directory under the workdir.
- `dec verify env show` and `list` render the value if present.
- The SHACL shape for `VerificationEnvironment` adds an optional `dec:fixtureSource` predicate (cardinality 0..1, datatype `xsd:string`, min-length 1).

### Safety class compatibility

`dec:fixtureSource` is allowed on any safety class. The blast-radius implications are:

- `isolated`: fixture writes are confined to the tempdir; no concern.
- `shared-non-destructive`: fixtures may still copy a tree, but the env's `allowedOps` constrain what steps may do with it.
- `production-readonly`: a fixture would be unusual but not forbidden; the safety check on the graph's steps remains the load-bearing gate.

The predicate is purely declarative — it says "before steps execute, this tree is present". Whether anything in that tree is dangerous depends on the steps' `requiredOps`, not on the fixture itself.

## Rejected alternatives

- **Graph-native `dec:VerificationFixture` artifact type.** A fixture as a typed artifact with id, content hash, and a file manifest, stored under `.dec/verify/fixture/<FIX-NNN>/` + `<FIX-NNN>.ttl`. Rejected for now — adds a new artifact type, SHACL shape, CLI surface, and minting logic (a slice the size of FT-035..FT-040 collectively), without a load-bearing driver for graph-side queryability. Git already content-addresses the tree. If a future driver emerges (multi-repo fixture sharing, fixture composition, fixture lifecycle), promoting a path reference into a typed artifact is a mechanical refactor.
- **Inline tar/base64 in `dec:setup`.** Rejected — bloats the env Turtle, defeats diff review, defeats incremental editing of the fixture.
- **Embed fixtures as compiled-in binary assets** (per [ADR-007](ADR-007)'s pattern for the base ontology). Rejected — fixtures are test data, not distribution data. Recompiling `dec` to edit a fixture is the wrong feedback loop.
- **Make `dec:fixtureSource` an absolute path or URL.** Rejected — defeats reproducibility (different hosts have different absolute paths) and would require network or auth handling for remote sources. Repo-relative is the simplest correct contract.
- **Implement the runner contract in the same slice as the schema field.** Rejected — slice 1/2 ships authoring-only verification artifacts; the step executor is a slice 3+ concern. Booking the contract in this ADR without forcing executor work in this slice preserves the authoring-first cadence.

## Consequences

**Positive:**
- Flows with rich preconditions (`dec implement`, future `dec drive`) become verifiable in `isolated` envs.
- Fixtures are diffable, reviewable, version-controlled alongside the code they exercise.
- Stub workers via fixture `bin/` enable success-path verification without Claude auth or network egress.
- The env Turtle stays small and structural; complexity moves into the fixture tree where it belongs.

**Negative / accepted costs:**
- Two source-of-truth surfaces for a verification: the env Turtle (declares what to materialise) and the fixture tree (declares what gets materialised). The convention is simple but introduces an indirection.
- Fixture content isn't SPARQL-queryable. If we ever need that (e.g. "list every fixture that ships a stub code-writer"), we'll need the graph-native artifact this ADR rejected.
- The runner contract (steps 1–6 above) is not yet implemented. Until then, fixtures are declarative-only and dec-implement flows still can't execute end-to-end. This ADR books the contract; the executor implements it.

**Enforcement:**
- SHACL extends `VerificationEnvironmentShape` with the optional `dec:fixtureSource` predicate (max-count 1).
- The authoring CLI gains `--fixture-source` and validates the path exists under the workdir at commit time.
- The future graph executor enforces the runner contract.

## Status

Proposed. Bound to [FT-053](FT-053) (env schema + CLI surface) and to whichever future feature ships the step executor.
