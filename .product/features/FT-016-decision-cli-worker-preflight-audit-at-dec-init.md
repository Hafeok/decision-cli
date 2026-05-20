---
id: FT-016
title: 'decision-cli: Worker preflight audit at dec init'
phase: 1
status: planned
depends-on:
- FT-009
- FT-011
- FT-013
adrs:
- ADR-007
- ADR-008
- ADR-011
- ADR-012
- ADR-015
tests:
- TC-046
- TC-047
- TC-048
- TC-049
- TC-050
domains: []
domains-acknowledged:
  ADR-001: ADR-001 governs the oxi-events crate boundary; FT-016 does not cross or alter that boundary.
  ADR-002: ADR-002 (graph-as-state) governs persistence semantics; FT-016 reads/writes via the GraphWriter chokepoint and does not introduce event-sourced state.
  ADR-023: ADR-023 (feedback class controlled vocabulary) is implemented by FT-028; FT-016 produces no feedback artifacts.
  ADR-017: ADR-017 (action-interpretation pairing) is a Slice 2 structural requirement implemented by FT-021; FT-016 is out of scope for the pairing.
  ADR-018: ADR-018 (VerificationVerdict schema) is a Slice 2 artifact implemented by FT-020; FT-016 neither emits nor consumes verdicts.
  ADR-005: ADR-005 (value-stream scope) governs command-time scope; FT-016 runs inside an already-scoped command and does not introduce a new scope check.
  ADR-014: ADR-014 (fitness functions tracked as artifacts) is owned by FT-014/FT-015; FT-016 does not author or modify a fitness-function artifact.
  ADR-013: ADR-013 (code structure standards) applies workspace-wide; FT-016's code conforms to cargo/clippy/rustfmt and the module-size convention. ADR-013 itself is owned by FT-014.
  ADR-024: ADR-024 (feedback lifecycle state machine) is implemented by FT-027; FT-016 produces no feedback artifacts.
  ADR-025: ADR-025 (blocking vs non-blocking feedback semantics) is implemented by FT-032; FT-016 has no feedback to gate.
  ADR-016: ADR-016 (vertical-slice + compile-time SDP) is migrated by FT-018; FT-016's code is reorganised under that migration, not by this feature.
  ADR-022: ADR-022 (feedback as a first-class flow class) is a Slice 3 concern implemented by FT-026; FT-016 neither emits nor routes feedback.
  ADR-021: ADR-021 (action-interpretation agreement metric) is a Slice 2 fitness function implemented by FT-024; FT-016 produces no action/interpretation pair.
  ADR-027: ADR-027 (authority declarations in role catalog) is implemented by FT-030; FT-016 does not introduce or modify a role catalog entry.
  ADR-004: ADR-004 (PROV-O) governs session/event lineage; FT-016 produces no new Session or event type and inherits lineage from the harness.
---

## Description

When a user installs `dec` (e.g. `cargo install decision-cli`) on a clean host and runs `dec init`, the orchestration store is seeded correctly but the host has no `code-writer` worker on `$PATH`. The first `dec implement FT-XXX` then fails late with a confusing error:

```
dec implement failed: running code-writer worker:
code-writer worker exited with exit status: 1 and no stdout.
stderr: /usr/bin/python3: Error while finding module specification for
'code_writer.main' (ModuleNotFoundError: No module named 'code_writer')
```

The error surfaces deep inside the dispatch path (`crates/decision-cli/src/implement.rs:743`) — by the time the operator sees it, a session has been opened, a bundle has been written, and the failure mode looks like a worker bug rather than a setup gap. Today's resolution chain in `run_worker` (`crates/decision-cli/src/implement.rs:701-718`) is also opaque: there is no command that tells the operator what `dec` will try, in what order, or why a given attempt is expected to succeed.

This feature makes worker availability a **declared, audited prerequisite** of an initialised orchestration store. `dec init` learns to audit the workers required by the value stream definition; a new `dec doctor` command re-runs the audit on demand. Neither command installs anything — that remains the operator's job — but both surface the exact install commands needed for the current host, so the gap between "I ran `dec init`" and "I can run `dec implement`" closes in seconds rather than minutes.

This is the slice 1 bridge to ADR-015 (graph-native worker bindings, proposed). Once worker bindings become first-class graph artifacts, the resolution chain in this feature becomes the *discovery layer* that feeds the binding registry; until then it lives in `dec` as a deterministic search order plus an audit surface.

See `decision-cli-slice-1-bounds.md` §7 and ADR-008 (workers are stateless processes; the harness is responsible for finding and invoking them).

## Functional Specification

### Inputs

- The initialised orchestration store (post-FT-009 bootstrap) — provides the value stream and its declared role set.
- The current process environment: `$PATH`, `$CODE_WRITER_CMD`, `$DEC_WORKDIR` (per ADR-012).
- An embedded **worker manifest** compiled into the `dec` binary: a static table of `role → expected worker identity` entries (per ADR-007's embedding pattern). For slice 1 the manifest contains a single entry:

  ```toml
  [[worker]]
  role            = "code-writer"             # IMPLEMENTER_ROLE
  console_script  = "code-writer"             # binary name expected on $PATH
  python_module   = "code_writer.main"        # fallback module for `python3 -m`
  install_kind    = "uv-tool"                 # informational, drives the suggestion text
  source_hint     = "./workers/code-writer"   # path or package spec for the install command
  ```

  The manifest is read-only in slice 1; future slices may grow it as additional roles land.

### Outputs

- A **preflight report** printed by `dec init` (after successful bootstrap) and by `dec doctor` (on demand):

  ```
  Worker preflight:
    code-writer  OK   /home/op/.local/bin/code-writer (resolved via PATH)
    reviewer     —    role not active in current value stream
  ```

  When a required worker is missing:

  ```
  Worker preflight:
    code-writer  MISSING  no resolution found

  To install:
    uv tool install ./workers/code-writer            # workspace checkout
    uv tool install code-writer                       # published package (when available)

  Or set CODE_WRITER_CMD to an explicit invocation, e.g.:
    export CODE_WRITER_CMD="/path/to/.venv/bin/code-writer run-once"
```

- A **structured JSON form** of the same report behind `--format json` for scripting / CI use.
- An **exit code contract**: `dec doctor` exits non-zero when any required worker is missing; `dec init` succeeds even if workers are missing (a fresh host should be able to bootstrap before installing workers) but prints the audit prominently and exits with a distinct, non-zero advisory status — `0` for all-OK, `2` for "store initialised but workers missing".

### State

- Nothing new persisted to the orchestration store in slice 1. The audit reads the store (to enumerate active roles) but does not write back. The resolution result is recomputed on every invocation — the environment is the source of truth, the store is not yet (that's ADR-015).
- The embedded worker manifest is a build-time artifact; its sha256 is recorded in the `dec init` bootstrap session telemetry alongside the ontology hash already captured per ADR-007.

### Behaviour

1. **Role enumeration.** Query the store for roles referenced by the current value stream's value actions. For slice 1, this resolves to `{ code-writer }` for the bundled `engineering-development` stream.
2. **For each required role,** consult the embedded worker manifest to learn what to look for, then run the **resolution chain** (lifted unchanged from today's `run_worker`, ADR-aligned, and centralised in a `worker::resolve` module so `dec implement` and `dec doctor` share it):
   1. `--worker-command <cmd>` on the invoking subcommand (explicit override; not present on `init`/`doctor`, present on `implement`).
   2. `$<ROLE>_CMD` environment variable (e.g. `CODE_WRITER_CMD`), parsed as a shell command line.
   3. `which <console_script>` on `$PATH`.
   4. Sibling-workspace probe: when `dec` is run from inside the `decision-cli` source tree (detected by walking up for `Cargo.toml` + `workers/<role>/.venv/bin/<console_script>`), use the venv-installed binary. This is a convenience for contributors and is **off** for installed `dec` invocations outside a workspace.
   5. `python3 -c "import <python_module>"` probe — if it succeeds, `python3 -m <python_module>` is a valid invocation.
   6. Otherwise: unresolved.
3. **Report.** Render the table (text) or JSON document and, for any missing worker, print the install hints derived from `install_kind` + `source_hint`. The text format is opinionated: one row per role, fixed-width status column, resolved path on the same line, install hints in an indented block beneath any missing rows.
4. **`dec init` integration.** After the bootstrap session is committed, run the audit and append its summary to the standard init output. The audit failure does not roll back the bootstrap.
5. **`dec doctor` command.** A new top-level subcommand (per ADR-011's namespaced-subcommand shape). Runs the audit against the current working directory's store (per ADR-012). Flags:
   - `--format {text,json}` — default `text`.
   - `--role <role>` — restrict to a single role.
   - No mutation flags; `dec doctor` is read-only.
6. **Sharing with `dec implement`.** `run_worker` in `crates/decision-cli/src/implement.rs` is refactored to call `worker::resolve(role)` and act on the same result type. The behaviour for a successful resolution is unchanged. When resolution fails, `dec implement` aborts **before** opening a session, with the same install hints `dec doctor` would print — no half-state in the graph.

### Invariants

- The resolution chain is defined in one place (`worker::resolve`) and consumed by both `dec implement` and `dec doctor`. No second copy may live in `implement.rs` or elsewhere.
- `dec init` and `dec doctor` never invoke worker subprocesses for real work — they only probe with cheap, side-effect-free commands (`which`, `python3 -c "import …"`). No stdin is fed to a probed binary.
- The embedded worker manifest is the only authority for what `dec` looks for; the resolution chain reads from it rather than from inline string constants.
- `dec init` succeeds (exit 0 or advisory 2) regardless of worker presence — bootstrap and worker install are decoupled operations. A user must be able to `dec init` on a host with no Python at all, install workers later, and proceed.
- The audit never writes to the graph in slice 1 (deferred to ADR-015 once worker bindings exist).

### Error handling

- **Manifest entry malformed at build time** → static check in the build, treated as a build-time bug (mirrors ADR-007's stance on the embedded ontology).
- **Store unreachable** when `dec doctor` runs → exits with the same "not in an initialised working tree" error `dec` already uses elsewhere (ADR-012).
- **`python3 -c "import …"` raises** → treated as "module not importable", which is a normal negative result, not an error. The probe captures stderr only for the JSON output's `diagnostics` field and never propagates it as a `dec` failure.
- **`which` returns a stale entry pointing at a missing file** → resolution moves to the next step, and the stale entry is reported in the diagnostics (not as the resolved command).

### Boundaries

- `dec doctor` does NOT install workers. It tells the operator what to install and how.
- `dec doctor` does NOT verify worker *version* in slice 1 — only presence/invocability. Version pinning lands with ADR-015.
- `dec doctor` does NOT probe Claude Code authentication or model availability — that is a separate concern owned by FT-013's runtime error (`subscription_unavailable`).
- `dec init` does NOT block on worker availability. The orchestration store is initialised either way; the audit is advisory at init time and authoritative at implement time.
- The manifest is read-only; there is no `dec worker add` in slice 1. Operators wanting a non-default worker invocation use the `$<ROLE>_CMD` env var.

## Out of scope

- Graph-native worker bindings (`dec worker register`, persisted `(role, kind, command, version, hash)` tuples). Captured separately in ADR-015 (proposed).
- Automatic worker installation (`dec worker install`). Belongs to ADR-015's surface.
- Version compatibility checks between `dec` and worker versions. Requires worker bindings.
- A registry / catalogue server for worker packages. Deferred alongside ADR-007's deferred ontology registry.
- Probing Claude Code (`claude -p`) availability. Owned by FT-013; if added later, lands as a separate row in the doctor report, not part of the worker resolution chain.
- Auto-creating `~/.config/dec/workers.toml` user-registry — not introduced in slice 1; the `$<ROLE>_CMD` env var covers the same use case for one worker.
