---
id: FT-114
title: 'decision-cli: dec init auto-bootstraps any product-cli repo, with .env safety'
phase: 4
status: planned
depends-on: []
adrs:
- ADR-068
tests:
- TC-222
- TC-223
- TC-224
- TC-225
- TC-226
- TC-227
- TC-228
- TC-237
- TC-238
domains:
- api
- storage
domains-acknowledged:
  storage: Writes a generated value-stream .ttl under .dec/streams/ and a .env.example at the repo root; checks .gitignore. All writes are atomic and append-only per the existing fileops discipline; no schema migration.
  api: Extends the dec init CLI surface (no-arg discovery, --yes, --no-env-check); preserves the existing --from path bit-identically. No new API contracts beyond the documented flags + console output shape.
---

## Description

Today `dec init` requires `--from <stream.ttl>` pointing at a
hand-authored value-stream definition (decision-cli ships
`streams/decision-cli-development.ttl`). That ties the
orchestrator to a single project: every new repo that wants to
run `dec drive ship` first has to author its own analogous
`.ttl` — a high friction barrier that defeats the operator
intent of "any product-cli repo + Claude API key = ready to
orchestrate."

This feature makes `dec init` discover the current repo's
`.product/` graph and generate a sensible default value-stream
automatically: standard role catalog
(implementer, verifier, verify-graph-author), default
capability bindings (Scaleway endpoint resolver), subscriptions
derived from the TC runner types present in `.product/tests/`.
The hand-authored `--from <stream.ttl>` path stays as the
escape hatch for operators who need non-default subscriptions
or alternative capability bindings.

It also bakes credential hygiene into the same step: `dec init`
expects `SCW_SECRET_KEY` (and any future provider keys) in a
`.env` file at the repo root rather than ambient shell env;
verifies `.env` is listed in `.gitignore`; appends if missing
(with operator confirmation in TTY, automatic with `--yes`).
This closes the most likely accidental-credential-commit
footgun before it can happen.

The combined effect: `cd some-repo-with-dot-product/ && dec
init && dec drive ship --all` becomes the universal entry
point. No hand-authored `.ttl`, no ambient-env footguns.

## Functional Specification

### Inputs

CLI surface (the existing `dec init` command, extended):

- `dec init` (no args) — auto-discover from `.product/` in cwd
  (or walk up to find it, matching product-cli's discovery
  convention), generate value-stream, seed orchestration store,
  bootstrap `.env.example` + `.gitignore` safety check.
- `dec init --from <stream.ttl>` — existing behaviour preserved
  exactly. Skips auto-discovery; uses the provided value-stream
  verbatim. Still runs `.env` + `.gitignore` checks.
- `dec init --yes` — auto-confirm any prompts (append `.env` to
  `.gitignore` if missing, overwrite `.env.example` if present,
  etc.) Required for non-TTY use.
- `dec init --workdir <path>` — initialise at the named workdir
  rather than cwd. Discovery walks up from `<path>` for
  `.product/`.
- `dec init --no-env-check` — skip the `.env` + `.gitignore`
  safety pass. For operators who manage credentials another way
  (CI secrets, vault, etc.). Off by default.

### Outputs

**Generated value-stream** —
`.dec/streams/<repo-name>-development.ttl`:

- Subject IRI scheme:
  `https://decision-cli.dev/ns/streams/<repo-name>-development`.
- Role catalog: implementer, verifier, verify-graph-author
  (the slice-1 defaults). Authority declarations match
  decision-cli's current role catalog.
- Capability bindings: default Scaleway endpoint resolver, with
  `SCW_SECRET_KEY` named as the required secret (so the env
  check has something concrete to look for).
- Subscriptions: scanned from `.product/tests/`. For every
  unique `runner:` value found across the TC corpus (e.g.
  `cargo-test`, `bash`, `pytest`), wire the matching subscription
  to dispatch that runner.
- Comment header naming the generator (this feature), the
  generation timestamp, and the source `.product/` path —
  audit-trail for "where did this come from."

**Generated config.toml** — `.dec/config.toml`, per ADR-068:

- Bootstrapped from ADR-068's initial inventory: `[driver]`,
  `[sweep]`, `[show]`, `[init]`, `[paths]` sections with every
  key set to its built-in default. The generated file is
  byte-stable (deterministic generation) so re-running `dec
  init` against an unchanged ADR produces an unchanged file.
- Includes header comment naming ADR-068 as the authority and
  the precedence-chain reminder
  (`# flag > DEC_* env > this file > built-in default`).
- Strict-parsed at every CLI startup; unknown keys, type
  mismatches, out-of-range values, and credential-shaped key
  names cause CLI startup to fail with a precise error
  (TC-238 pins the contract).
- Operator may delete the file entirely; the CLI falls back
  to built-in defaults across the board (the file is a
  preference layer, not required state).

**`.env.example`** — created at repo root if absent:

```
# Decision-CLI orchestrator credentials.
# Copy to .env and fill in your values. .env is gitignored.

# Scaleway Claude inference endpoint (default provider).
SCW_SECRET_KEY=

# Future providers: add here as their capability bindings ship.
```

**`.gitignore`** — checked at repo root:

- If `.env` (exact line, optionally with leading slash) is
  already listed: no change, no prompt.
- If missing and stdin is a TTY: prompt
  `.env is not in .gitignore — append? [Y/n]`. On `Y` append
  `.env` as a new line.
- If missing and stdin is non-TTY (or `--yes`): append without
  prompting.
- If no `.gitignore` exists at all: create one with `.env` as
  its only line.

**Orchestration store** —
`.dec/store/orchestration.nq`:

- Same shape and contents as today's `dec init` produces from a
  hand-authored stream — quad-set seeded from the generated (or
  provided) value-stream, ontology embedded, capability
  bindings materialised.

**Console output** — what got created and why:

```
discovered .product/ at /home/user/some-repo
generated  .dec/streams/some-repo-development.ttl
            roles: implementer, verifier, verify-graph-author
            subscriptions: cargo-test, bash, pytest
            capability bindings: scaleway (needs SCW_SECRET_KEY)
seeded     .dec/store/orchestration.nq
.env       not present — copy .env.example to .env and fill it in
.gitignore .env listed ✓
ready: run `dec drive ship --all` once .env is populated
```

### State

Persists everything the existing `dec init` persists, plus the
generated value-stream `.ttl` file. The `.env.example` is
committed; the `.env` itself is operator-managed and gitignored.

### Behaviour

1. **Discover `.product/`.** Walk from `--workdir` (default
   cwd) upward looking for a directory containing
   `product.toml`. If not found, exit non-zero with
   `"No .product/ graph discovered. Run `product feature new ...`
   first or pass --workdir <path>."`.
2. **Generate or load value-stream.**
   - If `--from <stream.ttl>` given, load that file directly and
     skip generation. Continue at step 4.
   - Otherwise, generate the default stream:
     a. Read `.product/tests/*.md` frontmatter; collect the set
        of unique `runner:` values.
     b. Read `.product/product.toml` for the repo-name hint
        (`name` field if present, else basename of the
        `.product/`'s parent).
     c. Compose `.dec/streams/<repo-name>-development.ttl`
        with the default role catalog, the default capability
        bindings, and one subscription per runner type observed.
   - Write the generated `.ttl` to disk (atomic write).
3. **Confirm with operator (TTY only).** Print the discovered
   facts and what will be generated; if stdin is a TTY and
   `--yes` not set, prompt `Proceed? [Y/n]`. In non-TTY use,
   continue without prompt.
4. **Seed orchestration store.** Existing logic applies —
   load ontology, materialise capability bindings from the
   value-stream, persist `.dec/store/orchestration.nq`.
5. **`.env.example` bootstrap.** If
   `<repo-root>/.env.example` does not exist, write the
   template described in Outputs. If it exists, leave it
   alone (operator may have customised).
6. **`.gitignore` safety check.** Per the Outputs spec. Final
   state of the file is one of three: unchanged (`.env` was
   already listed), one line appended, or newly created. Log
   which outcome happened.
7. **`.env` presence check.** If `<repo-root>/.env` does not
   exist, print the "not present — copy .env.example" line
   (the orchestrator can run dec init, but workers will fail
   without credentials). If `.env` exists, leave it alone
   and don't read its contents (no security surface for
   logging keys back).
8. **Print readiness summary.** The block shown in Outputs.
   Exit zero unless a step failed.

### Invariants

- The generated value-stream is deterministic given identical
  `.product/` input + repo-name. Two invocations on the same
  repo state produce byte-identical `.ttl`. Tests can pin the
  shape by fixture.
- The on-disk state after `dec init` is the same whether the
  stream was generated or loaded via `--from`: the
  orchestration store, the `.env.example`, the `.gitignore`
  check all run regardless of the source.
- `dec init` never reads `.env`. The harness reads it at
  command-time when workers need it; init only verifies its
  *gitignore status*, not its contents.
- `.gitignore` modifications append, never rewrite. If the
  file exists with arbitrary other entries, those are
  preserved.
- The hand-authored `--from` path is bit-identical to the
  existing slice-1 behaviour. This feature is strictly
  additive on the auto-discover path; it does not change what
  `dec init --from streams/decision-cli-development.ttl`
  produces today.
- Idempotent: re-running `dec init` on a bootstrapped repo
  re-generates the `.ttl` (overwriting if the source has
  changed) but does NOT corrupt the orchestration store. The
  store-seed step is itself idempotent per existing init
  invariants.

### Error handling

- No `.product/` discoverable: exit non-zero with the
  remediation hint named above. No partial init artifacts left
  behind.
- Value-stream generation fails (couldn't parse a TC, couldn't
  determine a runner type, etc.): exit non-zero with the
  offending file path. No partial `.ttl` written.
- `.gitignore` is a directory (typo or symlink): exit non-zero
  with the path; do not attempt to rewrite.
- Operator says `n` at the TTY prompt: exit non-zero with
  "aborted" message, no side effects committed.
- `.env.example` write fails (filesystem full, permission
  denied): exit non-zero with the path. Other artifacts that
  *did* write successfully are not rolled back (they're
  individually safe to have).
- Store-seed step fails after the `.ttl` was written: leave
  the `.ttl` on disk (it's the input the next attempt will
  use); exit non-zero with the seed error.

### Boundaries

- This feature does NOT introduce a new value-stream schema or
  ontology. The generated `.ttl` uses the existing
  decision-cli ontology vocabulary.
- This feature does NOT modify worker prompts to adapt them
  per-project. Workers stay as-is; bundle quality on
  non-decision-cli projects is whatever the existing prompts
  produce. Per-project prompt tuning is a future feature.
- This feature does NOT manage `.env` contents — never reads,
  never writes, never validates the values inside. Only the
  *existence* of `.env` and its gitignore status matter here.
  Operator owns credential lifecycle.
- This feature does NOT auto-detect non-Scaleway providers. If
  the operator wants Anthropic-direct or any other endpoint,
  they author a `--from <stream.ttl>` with custom capability
  bindings. Default is opinionated.
- This feature does NOT auto-detect runners outside the
  product-cli supported set. If a TC has an unknown runner
  type, the generator emits a warning and a subscription
  scaffolded as `runner: <unknown> # TODO: wire`. The operator
  fills in the gap.

## Out of scope

- **Multi-provider credential discovery.** Today's default is
  Scaleway only. Adding Anthropic-direct, Bedrock, Vertex, or
  any other endpoint as a first-class default belongs in a
  future feature that extends the generator's capability-
  binding template set.
- **Per-project worker prompt profiles.** The original FT-114
  pitch was "make workers project-portable via a profile
  knob." That's deliberately deferred — bundle quality
  improvements are independently valuable, but they're not
  blocking the goal of "any product-cli repo can run the
  orchestrator." If workers struggle on a specific repo's
  conventions, address with per-bundle teaching (the existing
  TC body discipline + LIFT THE RUNNER patterns) before
  introducing a project-profile schema.
- **`.env` value validation** (calling the provider's auth
  endpoint to confirm the key works). Useful future
  ergonomics layer; not a hygiene requirement.
- **Loading `.env` automatically at every `dec` invocation.**
  The current convention is operator-sourced (`source .env`
  or env-wrapper tooling); changing that is a separate
  decision (probably an ADR) that should be its own feature.
  Here we only guarantee `.env` exists at the right path and
  is gitignored — wiring it into the runtime is the next
  step.
- **`.product/`-graph repair or migration.** If the discovered
  `.product/` is from an older product-cli version with a
  different schema, this feature errors with the version
  mismatch — it does not attempt schema migration.
- **Removing the `--from <stream.ttl>` escape hatch.** Always
  available; documented as the path for non-default
  subscriptions / non-default capability bindings.
