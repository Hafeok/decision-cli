---
id: FT-180
title: 'decision-cli: symbol-level write tools and cluster-sandbox code intelligence (worktree overlay)'
phase: 5
status: planned
depends-on:
- FT-179
adrs:
- ADR-092
- ADR-071
- ADR-088
tests:
- TC-465
- TC-466
- TC-467
- TC-468
domains:
- security
- workers
domains-acknowledged:
  ADR-087: Write tools route through the worker safety module; audit emission and verdict consumption are unchanged (better in-session signal only reduces repair rounds).
  ADR-083: Symbol-write tooling is dispatch infrastructure, not a binding-level tech detail.
  ADR-082: Overlay querying serves cells below the TaskType layer; archetype contracts unaffected.
  ADR-084: No archetype ships or changes status; seam audits untouched.
  ADR-081: No CLI enumerate/lookup verb pair is added; worker-facing tools only.
---

## Description

Slice two of [ADR-092](ADR-092), gated on slice one's witnessed value: symbol-level write tools (`replace_symbol_body`, `insert_after_symbol`, `insert_before_symbol`, `rename_symbol`, `format_document`) served by the same run-scoped LSP service, and cluster-sandbox support via the [FT-172](FT-172) construction — a temporary worktree of HEAD with the cell's sandbox outputs grafted as an overlay, so cells query code intelligence over the workspace their output will actually join.

## Functional Specification

### Inputs

- The FT-179 service, cache, and client plumbing.
- The FT-172 worktree-overlay graft logic, extracted from the compile-probe script into a harness module both consumers share.
- Role catalog: write tools seed to the implementer role only; verifier never gains them (ADR-092 §5).

### Outputs

- Five write tools in the worker registry, each routed through the [ADR-071](ADR-071) safety module (containment, secrets blocking) like `write_file`.
- Post-edit pipeline per write: apply at the symbol range resolved from the cache, notify the LSP (`didChange`), invalidate the file's cache entry, return `{symbol, range, lines_changed}`.
- Overlay support: for cluster dispatches the service roots at a temp worktree (HEAD + sandbox overlay), refreshed when a cell's output changes between repair rounds ([FT-171](FT-171)).
- Symbol-not-found errors carry the file's available symbols (same contract as reads).

### State

- Temp worktrees under the system temp dir, removed unconditionally after the run (mirrors FT-172's hygiene).
- No new graph state beyond catalog seeds for the write tools.

### Behaviour

1. Symbol writes resolve the target range via the cache/LSP, apply the edit, and leave diagnostics immediately consistent for a follow-up `get_diagnostics`.
2. `rename_symbol` applies the LSP's workspace edit across all referencing files in the (work)tree — semantic, not textual.
3. For cluster cells, reads and writes operate on the overlay worktree; placement ([FT-170](FT-170)) still consumes the sandbox as today — the overlay is the *query* surface, the sandbox remains the *output* surface.
4. Mechanical cells are untouched (no tool surface, [ADR-088](ADR-088) §6).

### Invariants

- Every write tool passes the same containment and secrets gates as `write_file`; symbol resolution never bypasses [ADR-071](ADR-071).
- A symbol edit touches only the resolved range; bytes outside it are preserved exactly.
- Sandbox files are never mutated by the overlay machinery; reads cannot dirty a cell's output.

### Error handling

- rust-analyzer declining or fumbling a write-shaped request (its weaker territory per ADR-092) → structured tool error; the model falls back to `write_file`. Never a partial multi-file edit: workspace edits apply atomically or not at all.
- Overlay construction failure → cluster dispatch proceeds without semantic tools (fail-open, recorded), mirroring FT-179's degradation.

### Boundaries

- `apply_code_action` is deferred until rust-analyzer's code-action quality is assessed against real sessions.
- No change to audit/repair flow: better in-session signal should *reduce* repair rounds, but the audit remains the gate ([ADR-087](ADR-087)).

## Out of scope

- Write tools for the verifier role under any configuration.
- Multi-language write support (Rust only until a feature demands more).
- Replacing `write_file` — it remains the general-purpose mutation path and the fallback.
