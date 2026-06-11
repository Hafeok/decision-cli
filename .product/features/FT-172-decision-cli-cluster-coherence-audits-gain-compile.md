---
id: FT-172
title: 'decision-cli: cluster coherence audits gain compile and canonical-namespace checks for Rust-emitting TaskTypes'
phase: 4
status: planned
depends-on:
- FT-170
adrs:
- ADR-080
tests: []
domains: []
domains-acknowledged: {}
---

## Description

The FT-147 cluster run exposed two audit blind spots that let "succeeded" output reach the operator unpromotable: the emitted Rust **did not compile** (const `NamedNode`, `derive(Eq)` over `f32`, a phantom `Provenance` type, consts used as match patterns), and the vocabulary used a **worker-invented IRI namespace** (`decisionframework.com` instead of the canonical `https://decision-cli.dev/ns#`). The coherence audit checks structure (files exist, SHACL covers struct fields, round-trip tests present) — all of which the broken output passed.

This slice adds the two checks the operator had to perform by hand to `cluster-audit-add-artifact-type.py` (and the shared audit conventions for future Rust-emitting TaskTypes):

1. **Canonical namespace** — every emitted `.rs` and `.ttl` that declares IRIs must use `https://decision-cli.dev/ns` prefixes; any other absolute IRI base in a `dec`-vocabulary position fails the audit with the offending file:line.
2. **Compile probe** — the emitted Rust must type-check when grafted into the target crate. The audit copies the sandbox's `crates/` overlay into a temporary clone of the workspace (git worktree of HEAD + overlay), wires the module declarations the cell set prescribes, and runs `cargo check -p dec-ontology` with a hard timeout. Audit fail carries the rustc diagnostics.

## Functional Specification

### Inputs

- `scripts/checks/cluster-audit-add-artifact-type.py` (FT-141) and its fixture-dir CLI contract.
- The sandbox layout guaranteed by [FT-170](FT-170) (cells at their declared `output_path`s).
- The workspace git checkout (for the compile-probe worktree).

### Outputs

- Two new checks in the audit script: `canonical_namespace` and `compile_probe`, emitting the existing `FAIL check=<name>: <detail>` convention (which [FT-171](FT-171) maps back to cells: namespace → `iri_module_consts`/`shacl_shape`; compile → all Rust cells).
- `CoherenceAuditSpec.timeout_seconds` for `add-artifact-type` raised to accommodate the compile probe (worktree + `cargo check` against a warm target dir).

### State

- No graph-resident changes. The temp worktree is created under the system temp dir and removed unconditionally after the probe.

### Behaviour

1. Structural checks run first (cheap, unchanged); `canonical_namespace` next; `compile_probe` last (expensive, only reached on an otherwise-clean sandbox).
2. Exit codes keep the runner contract: 0 pass, 1 fail, 2 unrunnable (e.g. cargo missing).

### Invariants

- A sandbox that passes the audit compiles against HEAD and speaks only the canonical namespace — operator promotion becomes relocation plus review, not repair.

### Error handling

- Worktree setup or cargo invocation failures are `unrunnable` (exit 2), never silent passes.

### Boundaries

- Only the `add-artifact-type` audit gains the compile probe in this slice; other TaskTypes' audits adopt it as they prove the need (their emissions are Python or single files today).
- Prompt changes to prevent the namespace drift at the source are welcome but not required — the audit is the gate.

## Out of scope

- Running the emitted round-trip tests in the probe (compile-only; the tests run post-promotion via the feature's TCs).
- Auditing semantic vocabulary choices beyond the namespace base.