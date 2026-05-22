---
id: FT-IMPL-001
title: Stub feature for dec implement verification
phase: 1
status: planned
depends-on: []
adrs: []
tests: []
domains: []
---

## Description

Trivial feature_spec used as input to `dec implement` in verification flows. When `dec implement FT-IMPL-001` runs against this fixture, the bundled stub code-writer (`bin/code-writer`, forced into `CODE_WRITER_STUB=1` mode) emits a deterministic `CodeChange` writing `stub-output/FT-IMPL-001.md` into the workspace instead of invoking Claude.

The spec body exists only so `product context FT-IMPL-001` can assemble a non-empty bundle. The contents are not load-bearing for any verification step.

## Functional Specification

### Inputs

The bundle assembled by `product context FT-IMPL-001 --depth N`.

### Outputs

A `CodeChange` artifact whose `files_written` list contains exactly `stub-output/FT-IMPL-001.md`.

### State

Stateless. The stub runner writes one file and returns.

### Behaviour

Deterministic. The same dispatch payload produces the same `CodeChange` byte-identically.

### Invariants

- `CODE_WRITER_STUB=1` is set in the worker's environment (the fixture's `bin/code-writer` shim forces this).
- No network calls.
- No Claude auth required.

### Error handling

If the worker is not invoked through the shim, `code-writer` falls back to the real `claude -p` runner — outside this fixture's scope.

### Boundaries

In scope: providing a stable target for `dec implement` end-to-end verification.

Out of scope: doing anything useful with the resulting `CodeChange`. Real implementation flows use the host's `.product/` graph.

## Out of scope

- Production-quality feature_spec content.
- Multiple dispatches against this same feature id (the stub runner is idempotent but the verification graph treats each run as a fresh world).
