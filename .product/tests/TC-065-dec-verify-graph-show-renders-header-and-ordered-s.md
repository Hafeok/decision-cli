---
id: TC-065
title: dec verify graph show renders header and ordered step list
type: exit-criteria
status: passing
validates:
  features: []
  adrs: []
phase: 2
runner: cargo-test
runner-args: -p decision-cli --test tc_065_dec_verify_graph_show_renders_header_and_ordered_s
runner-timeout: 120
last-run: 2026-05-24T19:14:02.761001272+00:00
last-run-duration: 0.3s
---

## Description

[FT-043](FT-043)'s `dec verify graph show <VG-NNN>` returns the graph header (verifies, environment) and the steps in `dec:steps` rdf:List order. Both `--format text` and `--format json` are supported; the MCP twin returns the same structured value.

## Acceptance Criteria

1. **Header render.** `dec verify graph show VG-NNN --format text` prints, in order: id, `Verifies:` line (FT or TC), `Environment:` line including the env's safety class, then a `Steps:` header.

2. **Step order matches storage.** A graph authored with three steps (`shell-command`, `sparql-assertion`, `file-assertion`) renders the steps in positions 1, 2, 3 in that exact order. Re-running the command repeatedly produces the same ordering byte-for-byte.

3. **Step summary line.** Each step row shows a 1-based position, the step kind, and a one-line summary of the kind's key field (e.g. `command="..."` for `shell-command`, `query="..."` truncated for `sparql-assertion`).

4. **JSON format.** `--format json` emits a graph document with `id`, `verifies`, `environment`, `steps` (array of full step documents including every field); array order matches the rdf:List.

5. **Round-trip.** Reserialising the JSON output back to Turtle yields canonically equal Turtle to the on-disk file (modulo blank-node renaming).

6. **MCP parity.** `dec_verify_graph_show` with `{ id, format: "json" }` returns the same JSON document as the CLI.

7. **Unknown id.** `dec verify graph show VG-999` exits 1 with `Error::ArtifactNotFound { kind: "VerificationGraph", id: "VG-999" }`.

8. **`${name}` preserved in render.** A step whose field carries `${earlier_capture}` displays the literal placeholder text in both formats — no resolution, no warning.

## Fixture

- Tempdir with one graph carrying steps of at least four kinds (including a `capture` step and a step referencing `${name}`).

## Out of scope

- DAG renderer (slice 3+).
- Diff against another graph.