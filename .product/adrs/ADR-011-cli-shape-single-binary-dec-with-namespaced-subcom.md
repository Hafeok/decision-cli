---
id: ADR-011
title: 'CLI shape: single-binary dec with namespaced subcommands'
status: accepted
features: []
supersedes: []
superseded-by: []
domains: []
scope: feature-specific
content-hash: sha256:e898ce4a347365f9bc29da61aefb0d31e0e8c8e43c06951b554fa05d02eb04a3
---

## Context

The `dec` CLI grows over many slices: bootstrap, identity / health, triggering, inspection, goal-driven dispatch, watchers, schedulers, policy management, model management, checkpoints, cross-stream commands, and eventually the absorbed `product` subcommand surface.

Two shapes are possible:

1. **Multiple binaries.** `dec`, `dec-events`, `dec-session`, etc. The Unix small-tools tradition.
2. **Single binary with namespaced subcommands.** `dec init`, `dec events tail`, `dec session show`, `dec drive ship FT-007`. The pattern of `az`, `gcloud`, `kubectl`, `git`, `cargo`.

decision-cli is an orchestrator: many commands operate on shared state (the orchestration store) and share argument parsing, configuration discovery, logging, and store-open semantics. Splitting into multiple binaries would force every binary to re-implement that shared infrastructure or share it via a library that the binaries then duplicate-link. The orchestrator pattern is dominant in modern tooling for a reason.

See `decision-cli-slice-1-bounds.md` §9 and `CLAUDE.md` "CLI vocabulary."

## Decision

decision-cli ships as a **single binary `dec`** with **namespaced subcommands**.

The vocabulary is structured per-slice:

- **Slice 1:** `dec init`, `dec status`, `dec health`, `dec implement FT-XXX`, `dec events tail`, `dec events since <seq>`, `dec session list`, `dec session show <id>`, `dec session log <id>`.
- **Later slices add:** `dec drive <goal> <artifact>` (goal-driven dispatch), `dec dispatch role <role> <artifact>` (manual single-role escape), `dec watch <role>` (standing role), `dec schedule <role> --interval <duration>` (periodic role), `dec product <subcommand>` (engineering authoring, when product-cli is absorbed), `dec goal`, `dec role`, `dec model`, `dec policy`, `dec subscription`, `dec checkpoint`, `dec stream`.

The shorthand `dec implement` is a documented convenience: for any single-role direct dispatch, the role's verb form is a valid shortcut for `dec dispatch role <role> <artifact>`. This shorthand is preserved through later slices.

CLI arguments parsed via `clap` (or equivalent). Exit codes follow `sysexits` conventions where applicable.

## Consequences

**Positive:**

- One installable artifact. `cargo install decision-cli` ships everything.
- Shared infrastructure (config discovery, store opening, logging) is implemented once.
- Discoverability: `dec --help` shows the entire surface; `dec <noun> --help` shows the namespace.
- Conformance to dominant tooling conventions reduces user surprise.

**Negative / accepted costs:**

- Binary grows with feature surface (not a real cost at slice 1 scale).
- A single subcommand bug can occasionally affect adjacent subcommands through shared code (mitigated by good module boundaries).

**Explicit slice 1 omissions:**

- `dec drive`, `dec watch`, `dec schedule`, `dec dispatch role`, `dec checkpoint`, `dec stream`, `dec product` are **not implemented** in slice 1 (per §6.2 and ADR-010).

## Status

Accepted. Governs FT-012 (slice 1 CLI commands) and the CLI vocabulary outlined in `CLAUDE.md`.
