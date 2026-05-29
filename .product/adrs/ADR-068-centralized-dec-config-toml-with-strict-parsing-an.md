---
id: ADR-068
title: Centralized .dec/config.toml with strict parsing and DEC_* env override precedence
status: accepted
features:
- FT-114
supersedes: []
superseded-by: []
domains:
- api
- storage
scope: domain
---

**Status:** Accepted

## Context

Operator-tunable defaults across the `dec` CLI surface have
been accumulating ad-hoc as flags on individual subcommands —
`--max-iter 6`, `--per-feature-timeout 600`, `--bench
BNCH-002`, `--format text` — each invocation typing them out
again. As more knobs land (FT-110's cycle-buffer cap, FT-111's
sweep defaults, FT-113's watch interval, FT-115's parallel-
implementer cap), the lack of a persistent operator-preferences
layer turns the CLI into a "long bash alias" zone — every
operator carries their own `.bashrc` aliases or shell wrappers,
the team's tribal defaults are unreviewable, and adding a new
knob means asking "where should this default live?" with no
canonical answer.

Three relevant axes need a deliberate decision before any
feature adds its own knob storage:

1. **Where the defaults live** (per-workdir file vs global vs
   environment vs value-stream).
2. **Precedence** when multiple sources disagree (flag, env,
   file, code default).
3. **Parsing strictness** when an unknown key appears (typo
   catch vs forward-compat across CLI versions).

The status quo is "flags only" — every default has to be typed
out per invocation, and no persistent overrides exist.

## Decision

A centralised, per-workdir TOML file at
`<workdir>/.dec/config.toml` holds operator-tunable defaults.
The file is **strict-parsed**: any unknown key is a hard error
at CLI startup, not a warning. Configuration values are
overridable along a single deterministic precedence chain.

**Precedence (highest wins)**:

```
--flag  >  DEC_<UPPER_SNAKE> env var  >  .dec/config.toml  >  built-in code default
```

**Env var naming**: the env var name mirrors the TOML
key in `UPPER_SNAKE_CASE`, with section prefix collapsing into
the key. Examples:

| TOML | Env var |
|---|---|
| `[driver] max_iter` | `DEC_MAX_ITER` |
| `[sweep] per_feature_timeout_secs` | `DEC_SWEEP_PER_FEATURE_TIMEOUT_SECS` |
| `[sweep] auto_retire_failing_graphs` | `DEC_SWEEP_AUTO_RETIRE_FAILING_GRAPHS` |
| `[paths] worktree_root` | `DEC_PATHS_WORKTREE_ROOT` |

The mapping is mechanical: lowercase the env var, replace
underscores with `.` at the section boundary (first underscore
after `DEC_`) — the parser does this lookup; operators don't
need to memorise the rule.

**Strict parsing**: unknown keys, type mismatches, or
out-of-range values fail CLI startup with a precise error
naming the file path, the offending key, and what was expected.
Forward-compat across CLI versions is acknowledged as a known
limitation that a follow-up forward-merge tool will address —
the CLI never silently ignores config that an operator authored.

**Out-of-scope by design**:

- **Credentials** belong in `.env` (FT-114), not config.toml.
  The strict parser refuses any key containing `secret`, `key`,
  `token`, or `password` substrings as a tripwire against
  accidental commit (a config file is committable; `.env` is
  gitignored).
- **Per-feature / per-TC behaviour** belongs in the product
  graph, not config.toml. Per-artifact tuning that varies by
  ID is a different concern.
- **Provider URLs and capability bindings** belong in the
  value-stream `.ttl`. Those are architectural per-workdir
  declarations the orchestration store ingests; they're not
  operator-tunable in the runtime sense.
- **Internal tuning constants** (PAT-002's 8-slot ring buffer,
  byte limits inside SPARQL queries) stay as code constants.
  Surfacing them as config knobs leaks implementation detail
  and creates unbounded configuration surface.

## Initial configuration inventory

The first cut of `.dec/config.toml`. Future knobs are added by
amending this ADR (or superseding it with ADR-NNN), then
shipping the parser + tests in a feature spec.

```toml
[driver]
max_iter = 6                          # planner iteration cap
default_bench = "BNCH-002"            # used when --bench omitted

[sweep]
per_feature_timeout_secs = 600        # `dec drive ship --all` per-item bound
default_format = "text"               # text | tsv | json
max_concurrent_implementers = 1       # FT-115 parallel cap (slice-1 = 1)
auto_retire_failing_graphs = false    # opt-in pre-pass; flag stays --retire-failing-graphs

[show]
watch_interval_secs = 2               # `dec drive show --watch` poll cadence

[init]
default_provider = "scaleway"         # auto-bootstrap provider
default_secret_env = "SCW_SECRET_KEY" # env var name dec init names in .env.example

[paths]
worktree_root = ".dec/worktrees"      # FT-115
store_path = ".dec/store/orchestration.nq"
```

Every key has a documented built-in default (matching the
value shown). The config file may omit any key; missing keys
fall through to the built-in default.

## Rationale

**Per-workdir TOML** rather than `~/.config/dec/config.toml`:
operators may juggle multiple repos with different orchestration
profiles. Per-workdir defaults travel with the repo and get
committed alongside it (the file IS committable — no
credentials live there). A global config would force
operators to swap `dec` configs when switching repos; the TOML
chosen here is the operator-visible analog to the
value-stream's role catalog.

**Strict parsing** over lenient: typo catch wins. An operator
who writes `max_inter = 8` (typo) under the lenient rule gets
the built-in default silently, has no idea why their config
didn't take effect, and reaches the conclusion "the CLI is
ignoring me." Under strict parsing, CLI startup emits
`ConfigError::UnknownKey { key: "max_inter", file: ".dec/config.toml:3" }`
and exits non-zero, naming the suggested fix. Forward-compat
is a real concern (operators upgrading from CLI v0.6 → v0.7
with a key removed); the forward-merge follow-up addresses it
explicitly rather than by silent acceptance.

**TOML over YAML / JSON5 / Dhall**: ecosystem alignment.
Cargo's `config.toml` is the canonical pattern for Rust CLI
tooling; product-cli already uses TOML for `product.toml`.
Choosing the same format means operators reading either tool's
config see the same shape.

**Single precedence chain** rather than per-knob overrides:
predictable. Operators learn the chain once
(`flag > env > file > default`) and it holds for every key.
Per-knob override rules ("this one config takes precedence over
that one env var") creates an undebuggable maze; we accept the
constraint that a flag always wins.

## Consequences

- **Positive:** Operators carry their tuning across invocations
  without aliases or shell wrappers. Tribal defaults become a
  reviewable diff in the repo. Adding a new knob has a
  canonical home (extend this ADR + amend the inventory). The
  config file is committable so teams can share a tuned setup.
- **Positive:** Strict parsing turns silent failures into
  immediate ones; the typo case bites the operator early
  instead of confusing them mid-run.
- **Positive:** The DEC_* env var convention lets CI overrides
  be set per-job without touching the committed file.
- **Negative:** Forward-compat across CLI versions is a known
  gap. The forward-merge follow-up needs to ship before a
  release that *removes* a config key, or operators upgrading
  hit a startup failure.
- **Negative:** Extra parsing step at CLI startup adds a small
  fixed cost. Mitigated by TOML's speed; config is read once
  per invocation and held in a `OnceCell`.
- **Negative:** Every new knob now requires an ADR amendment
  (or a superseding ADR). Process overhead vs the ad-hoc
  "drop a flag" alternative. Accepted because the discipline
  enforces the inventory's coherence.

## Rejected alternatives

- **Global `~/.config/dec/config.toml`.** Forces operators to
  swap configs when switching repos. Per-workdir wins because
  multiple workdirs with different bench/provider tunings is
  the common case, not the exception.
- **Lenient parsing (unknown keys = warning).** Silently
  ignored typos are unhelpful. The forward-compat concern is
  real but addressed by a separate forward-merge feature, not
  by sacrificing typo catch.
- **Per-subcommand config files** (`.dec/drive.toml`,
  `.dec/sweep.toml`). Multiplies surface; harder to author,
  harder to audit, harder to ship a single forward-merge tool.
- **Drop the file and use env vars only.** Loses the
  reviewable / committable / shareable property of a tracked
  file. Env vars are great for per-invocation overrides; they
  fail at "team default."
- **YAML or JSON5 instead of TOML.** TOML wins on ecosystem
  alignment (cargo, product-cli) and on operator readability
  for this shape (mostly flat sections with scalar values).
- **Embed config in the value-stream `.ttl`.** The value-stream
  is architectural per-workdir declaration (roles, bindings).
  Operator preferences are runtime concerns at a different
  layer. Mixing the two means an operator preference change
  forces a value-stream rewrite, which is the wrong axis.

## Cross-references

- **FT-114** ships the parser + initial config.toml generation
  as part of `dec init` auto-bootstrap.
- **Future feature** ships the forward-merge tool that handles
  key-removal across CLI versions (the strict-parsing
  consequence).
- **PAT-001** (Inspector + Planner trait pair) applies to the
  config-loading layer: a `ConfigSource` trait makes the
  precedence chain unit-testable against in-memory stubs.
