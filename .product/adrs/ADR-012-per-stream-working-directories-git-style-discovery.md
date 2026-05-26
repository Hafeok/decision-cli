---
id: ADR-012
title: Per-stream working directories (git-style discovery)
status: accepted
features:
- FT-009
- FT-016
supersedes: []
superseded-by: []
domains: []
scope: cross-cutting
content-hash: sha256:6f0d005faa4187608f7266fe42431dfa0008721f42c4ba85d97b49c9766e5734
source-files:
- scripts/checks/per-stream-workdir.sh
---

## Context

A decision-cli instance is scoped to a single value stream (see ADR-005). The question is: how does the running `dec` process **know** which stream it is acting on?

Three options:

1. **Global config.** A single `~/.config/dec/config.toml` declares the current stream. Switching streams means editing the config or running a `dec use <stream>` command.
2. **Per-invocation flag.** Every command takes `--stream=<name>`. No ambient state, but maximum typing and easy to forget.
3. **Per-directory config (git-style discovery).** Each stream lives in its own working directory containing `.dec/`. `dec` walks up from CWD to find the nearest `.dec/` directory.

The git model is dominant for tools that operate on a per-project scope. It has powerful properties: directory location is identity, switching scope is `cd`, the user is never ambiguous about which scope a command targets, and multiple scopes can coexist on one machine with no global state to contend.

See `decision-cli-slice-1-bounds.md` §3.5.

## Decision

Each value stream lives in its **own working directory** with a `.dec/` config and Oxigraph store. `dec` reads the current directory (walking up the tree if needed) to determine which stream it's acting on.

Concretely:

- The orchestration store path is `<working-dir>/.dec/store/`.
- The config (minimal in slice 1) is `<working-dir>/.dec/config.toml`.
- `dec init` creates `.dec/` in the current directory.
- All other commands walk up from CWD to find the nearest `.dec/`. If none is found and the command is not `dec init`, the command errors with a clear hint.
- Nested `.dec/` directories are a structural error (`StoreError::NestedRepo`).

**No global mode switching, no `--stream` flag for every command, no ambiguity** about which graph is being touched.

## Consequences

**Positive:**

- The mental model is git's: a working directory has a stream, `cd` changes context, no global state.
- Multiple streams coexist on one machine without contention.
- The stream identity is locatable: `pwd` and `ls .dec/` answer "what stream am I in?".
- Tools and humans share the same discovery rule.

**Negative / accepted costs:**

- Running `dec` from outside any working directory is an error (acceptable: clear hint included).
- Symlinked / mounted directories may surprise; walk-up follows real paths.
- A future cross-stream coordination command (deferred) will need to be explicit about which streams it spans.

**Cross-stream coordination (deferred):**

When an artifact crosses streams (e.g., an oxi-events release becoming a dependency in another stream), it crosses the bus carrying its source stream identity. The consuming stream ingests it via its Discovery process. Each stream's graph remains internally consistent; cross-stream linkage is explicit and audited via PROV-O. None of this lands in slice 1.

## Status

Accepted. Governs FT-009 (orchestration store and bootstrap) and FT-010 (active stream loaded from discovered `.dec/`).
