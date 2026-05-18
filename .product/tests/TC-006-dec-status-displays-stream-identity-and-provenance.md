---
id: TC-006
title: dec_status_displays_stream_identity_and_provenance
type: exit-criteria
status: passing
validates:
  features: []
  adrs: []
phase: 1
runner: bash
runner-args: tests/scripts/tc-006-status.sh
runner-timeout: 30
last-run: 2026-05-18T19:38:26.971613703+00:00
last-run-duration: 0.2s
---

## Purpose

Validates that `dec status` (FT-012) surfaces the value stream identity, definition source path, content hash, and base ontology version exactly as recorded by `dec init` (FT-008 / ADR-006), reading from the persisted PROV-O record (ADR-004).

Source: `decision-cli-slice-1-bounds.md` §11.2 exit-criteria #6 and the §3.7 display contract.

## Given

- A working directory previously initialized via `dec init --from ./streams/decision-cli-development.ttl` (see TC-002).
- The original `./streams/decision-cli-development.ttl` file unchanged on disk.

## When

```bash
dec status
```

## Then

stdout includes, at minimum:

1. The ValueStream name (`decision-cli-development`).
2. The definition source path (`./streams/decision-cli-development.ttl`).
3. The **content hash** of the source bytes (matches TC-002's recorded hash and the value computed from the file directly).
4. The terminal ValueAction URI (`va:shipped-feature`) and its provenance label (e.g., `bundled, ontology vX.Y.Z`).
5. The authorized goals list (`ship, land`).
6. The graph-store path (`./.dec/store`).

The matching display format is illustrated in §3.7:

```
Value Stream:      decision-cli-development
Definition:        ./streams/decision-cli-development.ttl (sha256:a3f2…)
Terminal Value:    va:shipped-feature (bundled, ontology v0.1.0)
Authorized Goals:  ship, land
Graph Store:       ./.dec/store
```

## Notes

- The session count / in-flight count parts of §3.7 are also expected to render but are tolerant of values 0 in a fresh init.
- This TC pairs with TC-002 to close the round-trip claim: what `dec init` records is exactly what `dec status` reports.