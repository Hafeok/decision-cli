---
id: FT-067
title: 'decision-cli: route verify-graph-author worker through shared resolver'
phase: 3
status: complete
depends-on:
- FT-016
- FT-048
- FT-049
adrs:
- ADR-007
- ADR-008
tests:
- TC-139
domains: []
domains-acknowledged:
  ADR-001: Application-layer feature; does not touch the oxi-events crate boundary.
  ADR-025: No Feedback artifact produced; blocking semantics not invoked here.
  ADR-012: Not a per-stream command; no working-directory walk-up involved.
  ADR-043: Feature does not introduce a new full-chain query.
  ADR-039: No new motivational predicate introduced by this feature.
  ADR-054: Feature does not call LiteLLM.
  ADR-018: No verification verdict artifact produced by this feature.
  ADR-035: Feature does not assemble a bundle that carries a stakes judgment.
  ADR-065: Worker runtime substrate decision (Dagger deferral) does not apply; this feature routes an existing Python worker invocation.
  ADR-044: Feature is a direct code refactor and not authored from a Brief.
  ADR-023: No Feedback artifact produced; controlled vocabulary not invoked here.
  ADR-027: No new role registered by this feature; existing verify-graph-author role unchanged.
  ADR-055: Feature does not introduce or extend the WorkerImage catalog.
  ADR-022: No Feedback artifact produced by this feature.
  ADR-047: Feature does not perform capability-tag-to-entry binding at dispatch time; worker resolution is filesystem/PATH-based here.
  ADR-038: No new artifact type introduced by this feature.
  ADR-017: Feature is not an action-interpretation pair; no paired interpretation session involved.
  ADR-014: No new fitness function introduced; TC-050's existing structural invariant is the relevant fitness signal.
  ADR-041: Feature does not write artifacts through GraphWriter.
  ADR-036: Feature does not touch the Capability/RoleBinding catalog.
  ADR-040: No new boundary artifact introduced by this feature.
  ADR-064: Feature does not call LiteLLM.
  ADR-024: No Feedback artifact produced; lifecycle state machine not invoked here.
  ADR-033: Worker resolution here is filesystem / PATH lookup, not capability-tag-based model routing.
  ADR-005: Worker-resolver routing is independent of value-stream scope.
  ADR-021: Feature does not produce an action-interpretation pair, so the agreement metric does not apply.
  ADR-004: Feature does not emit dispatch or session events.
  ADR-034: Worker-resolver routing does not invoke escalation tiers.
  ADR-037: Provider defaults are out of scope; this feature routes the worker spawn, not LLM provider selection.
  ADR-002: Feature ships a worker-resolver wiring change, not graph mutations.
---

## Description

`dec verify graph generate` (FT-049) is unusable today: it hardcodes `python3 -m verify_graph_author --stdin` (`crates/decision-cli/src/features/verify_graph_generate/worker.rs:111-125`), but the worker is shipped as a `uv tool install` package whose module is not importable by the system `python3`. Every invocation fails with `No module named verify_graph_author`. This blocks the planned dogfood pass that authors `VerificationGraph` artifacts for the existing 60-odd features.

The fix is mechanical: route the verify-graph-author worker through the shared resolver (`core::worker::resolve`, FT-016 / TC-050) the same way `dec implement` routes the code-writer. TC-050's structural invariant — "no second copy of the resolution chain lives anywhere in `crates/decision-cli/src/`" — is violated by the current state; the inline spawn in `verify_graph_generate/worker.rs` is exactly the second copy that the invariant forbids.

The verifier worker (FT-023, slice 2) is **not** in scope here: it is delivered via the outbox / SSE transport ([FT-022](FT-022)) rather than spawned by `dec`, so there is no Rust-side subprocess invocation to route. The single subprocess spawn that needs routing is `verify_graph_generate/worker.rs`.

## Functional Specification

### Inputs

- The embedded worker manifest (`core::worker::manifest`, FT-016) extended with a new `verify-graph-author` entry.
- The shared resolver (`core::worker::resolve`) — unchanged.
- The existing `verify_graph_generate::worker::invoke_worker` entry point and its `MockGuard` test harness.

### Outputs

- `dec verify graph generate FT-NNN --environment ENV-NNN` resolves the worker via the chain (override → `$VERIFY_GRAPH_AUTHOR_CMD` → `which verify-graph-author` → sibling-workspace probe → `python3 -m verify_graph_author`) and spawns whichever wins, appending `--stdin` to the resolved argv.
- `dec doctor` and `dec init` preflight reports include a `verify-graph-author` row (it joins `ACTIVE_ROLES_ENGINEERING_DEVELOPMENT`).
- Failures surface with the same operator-facing install hint shape that the implementer's missing-worker path already produces.

### State

- `crates/decision-cli/src/core/worker/assets/manifest.toml` gains a second `[[worker]]` table. This shifts `manifest_sha256_hex()` — by design; the SHA-256 is a fingerprint of the active manifest, not a frozen constant. Existing bootstrap sessions retain the old hash on their PROV-O records (historical truth), and freshly-initialised stores carry the new hash. No migration required.
- No on-disk schema changes. No store migrations.

### Behaviour

1. Add the entry to `manifest.toml`:
   ```toml
   [[worker]]
   role           = "verify-graph-author"
   console_script = "verify-graph-author"
   python_module  = "verify_graph_author"
   install_kind   = "uv-tool"
   source_hint    = "./workers/verify-graph-author"
   env_var        = "VERIFY_GRAPH_AUTHOR_CMD"
   ```
2. Mirror the entry in `MANIFEST` (the `&[WorkerEntry]` constant in `manifest.rs`), so the runtime view stays in sync with the embedded TOML (the existing `manifest_toml_mentions_each_runtime_entry` test enforces this round-trip).
3. Extend `ACTIVE_ROLES_ENGINEERING_DEVELOPMENT` to include `"verify-graph-author"` so preflight reports surface it.
4. Rewrite `features/verify_graph_generate/worker.rs::spawn_worker_subprocess` to:
   1. Call `worker::resolve(role_entry("verify-graph-author").expect("manifest"), ResolveInputs { override_command: None, workdir: None })`.
   2. On `Resolution::Resolved { argv, .. }`: build the `Command` from `argv[0]`, pass `&argv[1..]`, then append `--stdin`. Wire stdio as today.
   3. On `Resolution::Missing { diagnostics }`: return `HandlerError::Internal` with a `worker:`-prefixed detail that includes the diagnostics (matches the existing error category surfaced by FT-049's CLI tests).
5. Keep the thread-local `install_mock` harness intact — `MockGuard`, `invoke_worker`, and `subprocess_invocation_count` stay byte-identical. The change is entirely below `try_invoke_mock`.
6. No CLI surface change. `dec verify graph generate` keeps its current flags. (A `--worker-command` override could be added later; not required for this feature.)

### Invariants

- TC-050 (no second resolution chain) holds again: `verify_graph_generate/worker.rs` no longer contains `Command::new("python3")` literals.
- TC-046 (manifest hash is recorded on bootstrap) still holds — the hash value changes, but the recording invariant is unaffected.
- `verify_graph_generate::worker::install_mock` continues to short-circuit before any resolution attempt, so existing TC-079 / TC-080 / TC-081 / TC-082 mock-driven tests pass unchanged.
- `subprocess_invocation_count()` continues to count only real subprocess spawns. TC-080's "no subprocess on match-path" assertion holds.

### Error handling

- Missing worker → `HandlerError::Internal { detail: "worker: verify-graph-author not resolvable: <diagnostics>" }`. CLI exit code 1; stderr carries the install hint via the same renderer that `dec implement` uses for missing code-writer.
- Subprocess non-zero exit → unchanged (existing `check_subprocess_exit`).
- Bundle serialisation, stdin write, stdout parse errors → unchanged.

### Boundaries

- **In scope.** Manifest entry, `ACTIVE_ROLES_ENGINEERING_DEVELOPMENT` extension, `spawn_worker_subprocess` rewrite, one new unit test that asserts the resolver is consulted (sibling-workspace probe path).
- **Out of scope.** A `--worker-command` flag on `dec verify graph generate` (defer until an operator asks). Routing the verifier worker (FT-023) — it has no Rust-side subprocess. Renaming or repackaging the Python worker. Any change to the Python worker's stdin / arg surface (it already supports `--stdin`).

## Out of scope

- Replacing the manifest with a graph-native worker catalogue (ADR-015, slice 3+).
- Adding `dec doctor` rows for the verifier worker (it has no subprocess; the row would be aspirational).
- Backfilling old PROV-O bootstrap records with the new manifest hash (historical truth is correct as-is).

## References

- [FT-016](FT-016) — worker preflight audit at `dec init`; owns the shared resolution chain.
- [FT-048](FT-048) — verify-graph-author Python worker package.
- [FT-049](FT-049) — `dec verify graph generate` CLI/MCP (the broken caller).
- [ADR-007](ADR-007) — worker manifest fingerprinting on bootstrap.
- [ADR-008](ADR-008) — worker contract (stateless, bundle-in / artifact-out).
- TC-050 — "no second resolution chain" structural invariant.
