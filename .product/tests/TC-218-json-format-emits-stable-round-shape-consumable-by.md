---
id: TC-218
title: JSON format emits stable Round shape consumable by downstream tooling
type: scenario
status: passing
validates:
  features:
  - FT-113
  adrs: []
observes:
- stdout
phase: 4
runner: cargo-test
runner-args: tc_218_json_format_stable_shape
runner-timeout: 30
last-run: 2026-05-30T16:19:59.375908023+00:00
last-run-duration: 0.6s
---

## Description

The JSON format is the future web-dashboard's data contract
(see FT-113 §Out of scope). Pinning the shape now prevents
internal refactors from breaking the dashboard's parser
later.

## Acceptance Criteria

Cargo test:

1. Hand-construct a `Vec<Round>` covering one of each
   `Outcome` variant (VGA, implementer, verifier, stuck,
   done).
2. Call the JSON entry point (e.g. `render_json(&rounds)`);
   assert it returns valid JSON parseable by `serde_json`.
3. Assert the top-level is a JSON array.
4. Assert each element has keys exactly:
   `["round_index", "started_at", "elapsed_seconds",
     "state", "dispatch", "outcome"]`.
5. Assert `state` is an object with keys
   `["verdict", "impl_open", "vga_open", "graph_count"]`.
6. Assert `dispatch.role` is one of
   `"verify-graph-author" | "implementer" | "verifier"`.
7. Assert `outcome` is a serde-tagged enum variant: e.g.
   `{"type":"vga-produced","graph_id":"VG-167","steps":8,"pass":1,"fail":7}`
   or `{"type":"implementer-commit","sha":"a7b3c91","addressed":4,"remaining":3}`.
8. Snapshot the output against
   `tests/fixtures/drive-show-json.txt` so PR diffs surface
   shape drift.