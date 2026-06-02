---
id: TC-139
title: verify-graph-author worker is spawned through the shared resolver chain
type: exit-criteria
status: passing
validates:
  features: []
  adrs: []
phase: 1
runner: cargo-test
runner-args: tc_139_verify_graph_author_worker_is_spawned_through_the
runner-timeout: 120
last-run: 2026-06-02T12:37:16.266129871+00:00
last-run-duration: 0.7s
---

## Description

Validates the structural fix from [FT-067](FT-067): the verify-graph-author worker
must be spawned through the shared resolver chain (`core::worker::resolve`),
not via a hardcoded `python3 -m verify_graph_author` literal at the spawn site.

Three structural checks back the invariant:

1. **Manifest entry present.** `core::worker::role_entry("verify-graph-author")`
   returns the canonical `(console_script, python_module, env_var)` triple
   `(verify-graph-author, verify_graph_author, VERIFY_GRAPH_AUTHOR_CMD)`.
2. **Shared resolver honours the entry's env var.** `core::worker::resolve`
   with the verify-graph-author entry and `VERIFY_GRAPH_AUTHOR_CMD` set to
   a sentinel command returns `Resolution::Resolved { kind: Env, argv }`
   pointing at that command. This proves the role flows through the same
   chain as code-writer, not a bespoke lookup.
3. **`invoke_worker` end-to-end spawn.** When `VERIFY_GRAPH_AUTHOR_CMD`
   points at a sentinel bash script, `verify_graph_generate::worker::invoke_worker`
   actually executes that script (recorded via an argv log file) with
   `--stdin` appended (FT-067 §Behaviour) and pipes the bundle JSON to
   stdin. If the spawn site ever drifted back to an inline
   `python3 -m verify_graph_author`, the env override would be ignored
   and the sentinel script would not run — the AC-#3 asserts would fire
   loudly.

## Given

A clean tempdir; `VERIFY_GRAPH_AUTHOR_CMD` set to a bash sentinel script
that copies its stdin to a log file, records its argv to another log
file, and prints a stub `GraphProposal::Gap` JSON that echoes the input
`bundle_hash`.

## When

`decision_cli::verify_graph_generate::worker::invoke_worker(&bundle)`
is called with a minimal `VerifyGraphAuthorInputJson`.

## Then

- `core::worker::role_entry("verify-graph-author")` returns the
  expected triple.
- `core::worker::resolve` honours the env var and reports
  `ResolutionKind::Env` with the sentinel script as argv.
- `invoke_worker` returns the parsed stub proposal with `bundle_hash`
  echoed verbatim, the real subprocess counter increments by 1, the
  argv log contains `--stdin`, and the stdin log round-trips as the
  original `VerifyGraphAuthorInputJson`.