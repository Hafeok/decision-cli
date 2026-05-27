---
id: TC-194
title: dec loop show resolves source_session and addressing_artifact for each feedback and surfaces them by short id
type: exit-criteria
status: passing
validates:
  features:
  - FT-109
  adrs: []
phase: 3
runner: cargo-test
runner-args: tc_194_loop_show_short_id_resolution
runner-timeout: 60
last-run: 2026-05-27T10:44:23.462262169+00:00
last-run-duration: 0.5s
---

## Claim

The chain-walker behind `dec loop show` resolves PROV-O IRIs to operator-friendly short ids:

- `https://decision-cli.dev/ns/activity/verify-graph-run/VG-NNN/...` → `VG-NNN`.
- `https://decision-cli.dev/ns/graph/VG-NNN` → `VG-NNN`.
- `https://decision-cli.dev/ns/code-change/CC-NNN` → `CC-NNN`.
- `urn:dec:feedback:<uuid>` → first 8 chars of the UUID.
- Anything else → the full IRI.

## Scenarios

### Setup

- Three feedbacks for `FT-T194`:
  - `FB-1` source_session = `https://decision-cli.dev/ns/activity/verify-graph-run/VG-007/ts-1234`, addressing_artifact = `https://decision-cli.dev/ns/graph/VG-NEW-1` (verifier-class, addressed).
  - `FB-2` source_session same form for VG-009, addressing_artifact = `https://decision-cli.dev/ns/code-change/CC-FIX-2` (implementer-class, addressed).
  - `FB-3` produced, no addressing_artifact yet, source_session non-VG-pattern (free-form IRI).

### Test

`dec loop show FT-T194 --format json`. Assert:

1. `FB-1.source_session_short == "VG-007"`, `FB-1.addressing_artifact_short == "VG-NEW-1"`.
2. `FB-2.source_session_short == "VG-009"`, `FB-2.addressing_artifact_short == "CC-FIX-2"`.
3. `FB-3.source_session_short` falls back to the raw IRI; `FB-3.addressing_artifact_short` is absent / null.

### Boundary

- The text-format output prints the short ids in headers (`VG-007`, `CC-FIX-2`) while keeping the full IRI available in `--format json` for downstream parsing.