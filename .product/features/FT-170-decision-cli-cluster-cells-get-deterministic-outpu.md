---
id: FT-170
title: 'decision-cli: cluster cells get deterministic output placement — the harness writes resolved output_path, worker paths are advisory'
phase: 4
status: planned
depends-on: []
adrs:
- ADR-008
- ADR-080
tests: []
domains: []
domains-acknowledged: {}
---

## Description

Structural fix for the dominant failure mode of the witnessed FT-147 cluster runs (4 rounds, ~25 min and ~€0.56 each, where 3 rounds failed on one cell's path): the worker is asked to *transcribe* the harness-resolved `output_path` into its `write_file` call, and a probabilistic component doing a deterministic job intermittently drifts (witnessed: `ontology/archetype/shapes/…` instead of the declared `ontology/shapes/…`).

The harness already knows the answer before dispatch — [FT-166](FT-166) resolves `output_path` with parameter substitution. This slice makes the harness own *placement*, consistent with [ADR-008](ADR-008)'s worker contract (bundle in, artifact out; the harness owns writes): the worker produces content; wherever it lands in the cell's sandbox workspace, the harness relocates the cell's primary artifact to the resolved `output_path` after the cell completes. The audit's path expectations become structurally unfailable.

## Functional Specification

### Inputs

- `cluster_dispatch::run_cells` and `resolve_cell_output_path` (FT-166) in `crates/decision-cli/src/features/drive/cluster_dispatch.rs`.
- The per-cell sandbox workspace the code-writer writes into.
- `CellDecl.artifact_type` — used to recognise the cell's primary artifact by extension when the worker's chosen path differs.

### Outputs

- After each cell completes, the harness scans the cell's workspace for the produced artifact and **moves** it to the resolved `output_path` (creating directories). Cases:
  1. Worker wrote exactly the resolved path → no-op.
  2. Worker wrote one file of the right kind elsewhere → relocated, with a `tracing::info!` recording `from → to` (this drift signal feeds prompt tuning).
  3. Worker wrote nothing of the right kind → the cell fails immediately with a diagnostic naming the expected path (today this surfaces only later, at audit time).
  4. Worker wrote multiple candidate files → the cell fails with a diagnostic listing them; ambiguity is not silently resolved.
- The per-cell prompt keeps stating the output path (it helps content quality), but correctness no longer depends on the model honouring it.

### State

- No graph-resident changes. Sandbox layout after a successful cluster run is byte-identical to a run where the worker honoured every path.

### Behaviour

1. Cell completes → harness resolves `output_path` (FT-166) → placement scan → relocate/no-op/fail per the four cases above.
2. Cells with empty `output_path` (FT-145-era flat convention) keep the existing behaviour unchanged.
3. The coherence audit runs against the post-placement sandbox.

### Invariants

- After any successful cell, the file at the resolved `output_path` exists and is the cell's artifact.
- The harness never overwrites a previously placed *different* cell's output (collision is a cell failure, not a silent replace).

### Error handling

- Cases 3 and 4 fail the cell with `anyhow` diagnostics naming the expected path and the candidate set; the cluster's existing failure handling applies.

### Boundaries

- No worker-side changes; the code-writer contract is untouched.
- Audit content checks (compile, namespace — [FT-172](FT-172)) and retry granularity ([FT-171](FT-171)) are sibling slices.

## Out of scope

- Multi-artifact cells (no current TaskType declares one).
- Prompt-template changes beyond keeping the path mention.