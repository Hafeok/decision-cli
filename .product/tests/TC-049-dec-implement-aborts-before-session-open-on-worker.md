---
id: TC-049
title: dec implement aborts before session open on worker resolution failure
type: exit-criteria
status: passing
validates:
  features:
  - FT-016
  adrs: []
phase: 1
runner: bash
runner-args: tests/scripts/tc-049-implement-aborts-pre-session.sh
runner-timeout: 120
last-run: 2026-05-20T08:03:50.124890079+00:00
last-run-duration: 0.3s
---

## Description

The originating pain point for FT-016 is that worker absence used to surface deep inside the dispatch path — a session was already opened, a bundle was already written, and the operator saw a confusing `ModuleNotFoundError` rather than "install a worker." This TC pins the fix: when `worker::resolve` cannot resolve the required worker, `dec implement` must abort **before** opening a session, leaving the graph in exactly the state it had pre-invocation.

## Acceptance Criteria

Given an already-initialised `.dec/store/orchestration.nq` and a feature_spec FT-XXX in status `planned`:

1. **Pre-flight failure aborts.** With `code-writer` unresolvable (PATH scrubbed, `CODE_WRITER_CMD` unset, no `workers/code-writer/.venv/...`), `dec implement FT-XXX` exits non-zero.

2. **No new session in the graph.** A sha256 of `.dec/store/orchestration.nq` taken before and after the run is identical. Equivalently, a SPARQL query `SELECT (COUNT(?s) AS ?n) WHERE { ?s a prov:Activity }` returns the same count before and after.

3. **No bundle on disk.** No new file matches `/tmp/dec-bundle-FT-XXX-*.md` or any other tempfile path the worker would have received.

4. **No subprocess spawn.** A traced run shows zero `execve` calls to `code-writer` or `python3 -m code_writer.main` between `dec implement` start and exit.

5. **Diagnostic surfaces install hints.** stderr contains the same install-hint block `dec doctor` prints (the `uv tool install …` suggestion plus the `CODE_WRITER_CMD` override). stderr does **not** contain the legacy substring `code-writer worker exited with exit status` — i.e. the failure path is recognised, not stumbled into.

6. **Feature status unchanged.** `product feature show FT-XXX --format json` reports the same `status` value before and after the run.

## Fixture

- Tempdir with a fixture FT-XXX at status `planned`.
- Two passes for sanity: one with the worker resolvable (must succeed and reach the normal dispatch flow — regression guard), one with the worker unresolvable (the case this TC actually validates).

## Out of scope

- The text/JSON shape of the diagnostic (TC-048 covers JSON).
- Behaviour when the worker is resolvable but later crashes — that is FT-013's runtime error path, not FT-016.