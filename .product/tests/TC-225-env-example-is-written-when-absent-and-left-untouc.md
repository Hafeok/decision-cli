---
id: TC-225
title: .env.example is written when absent and left untouched when present
type: scenario
status: passing
validates:
  features:
  - FT-114
  adrs: []
observes:
- file
phase: 4
runner: cargo-test
runner-args: tc_225_env_example_bootstrap_or_preserve
runner-timeout: 30
last-run: 2026-05-30T16:21:42.181107422+00:00
last-run-duration: 0.6s
---

## Description

`.env.example` is the documented template (committed) operators
copy to `.env` (gitignored). The init step should bootstrap it
for fresh repos but never trample an operator's customised
version on re-runs.

## Acceptance Criteria

Cargo test:

1. **Bootstrap path.** Compose a temp repo with no
   `.env.example`. Call the env-bootstrap routine. Assert:
   - `.env.example` now exists.
   - It contains the substring `SCW_SECRET_KEY=`.
   - It contains the substring `.env is gitignored`.
2. **Preserve path.** Compose a temp repo where the operator
   has already authored `.env.example` with custom
   contents (`ACME_CUSTOM_PROVIDER_KEY=foo` and a comment).
   Call the env-bootstrap routine. Assert:
   - The file contents are byte-identical to what the operator
     wrote.
   - No backup file (`.env.example.bak` etc.) was created.
3. **`.env` is never written.** Confirm that no `.env` file
   (without the `.example`) is created by either path; the
   init step only manages `.env.example`.