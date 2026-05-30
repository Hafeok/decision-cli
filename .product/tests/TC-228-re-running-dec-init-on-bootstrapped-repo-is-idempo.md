---
id: TC-228
title: Re-running dec init on bootstrapped repo is idempotent and does not corrupt store
type: invariant
status: passing
validates:
  features:
  - FT-114
  adrs: []
observes:
- file
- exit-code
phase: 4
runner: bash
runner-args: tests/scripts/tc-228-init-idempotent.sh
runner-timeout: 60
last-run: 2026-05-30T07:54:48.214827485+00:00
last-run-duration: 0.5s
---

## Description

Operators re-run `dec init` for many reasons (after editing
the value-stream by hand, after adding a new TC runner type,
after script automation). Each re-run must be safe — same
result, no corruption, no silent loss of orchestration-store
state added by prior `dec drive ship` runs.

## Acceptance Criteria

Bash test:

1. Compose a temp repo with `.product/`, run `dec init --yes`
   the first time. Capture the resulting `.ttl` and
   orchestration `.nq` byte sizes.
2. Simulate a drive run: write a marker triple into the
   orchestration store
   (`<urn:tc-228-marker> <urn:p> "1" .`).
3. Run `dec init --yes` again. Assert:
   - Exit code 0.
   - The `.dec/streams/<repo>.ttl` byte size matches step 1
     (deterministic regeneration; the source `.product/`
     didn't change).
   - The orchestration store still contains the marker
     triple (proves init didn't wipe drive state).
   - The console output contains the substring `re-ran` or
     `already initialised` (operator-visible signal that
     init was idempotent, not a fresh seed).
4. Add a new TC to the `.product/tests/` directory with a
   new runner type (`deno-test`). Run `dec init --yes`. Assert:
   - Exit code 0.
   - The `.dec/streams/<repo>.ttl` byte size differs (new
     subscription appended).
   - The marker triple still exists.
   - The orchestration store now has a quad reflecting the
     new subscription (re-seed picks up the new stream).

The marker triple is the load-bearing assertion: it proves
that re-init is a stream-refresh, not a destructive reset.