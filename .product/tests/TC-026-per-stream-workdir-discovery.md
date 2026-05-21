---
id: TC-026
title: per_stream_working_directory_discovery
type: invariant
status: unrunnable
validates:
  features: []
  adrs:
  - ADR-012
phase: 1
runner: bash
runner-args: scripts/checks/per-stream-workdir.sh
runner-timeout: 60
last-run: 2026-05-20T11:41:36.841111001+00:00
failure-message: "ERROR: expected crates/decision-cli/src/scope/mod.rs (ADR-012 anchor)\n"
last-run-duration: 0.0s
---

## Purpose

Mechanical enforcement of **ADR-012 per-stream working directories**.
Asserts the scope loader still discovers `<workdir>/.dec/store/` —
the convention that makes the working directory the value stream's
identity (no `--stream` flag, no global registry).

## Given

- A working copy of decision-cli with `crates/decision-cli/src/scope/mod.rs`
  present.
- `bash` and `grep` available on `PATH`.

## When

```bash
scripts/checks/per-stream-workdir.sh
```

## Then

1. Exit 0 if the scope loader resolves `<workdir>/.dec/` and reads
   `orchestration.nq` from inside it.
2. Exit 1 if either the `.dec/` join or the `orchestration.nq` path has
   been removed (per-stream-workdir-discovery regressed).

## Formal Specification

⟦Γ:Invariants⟧{
  references(crates/decision-cli/src/scope/mod.rs, ".dec")
  references(crates/decision-cli/src/scope/mod.rs, "orchestration.nq")
  ¬ ∃ flag StreamRegistry ∈ crates/decision-cli/src/**.rs
}
