---
id: TC-226
title: Gitignore safety check appends .env line when missing in TTY mode with --yes
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
runner-args: tc_226_gitignore_safety_check_appends_or_preserves
runner-timeout: 30
last-run: 2026-05-30T16:41:05.759016390+00:00
last-run-duration: 0.4s
---

## Description

The `.gitignore` check is the credential-commit footgun
shield. It must catch the missing-line case automatically in
non-TTY use (CI, scripts) and prompt or auto-confirm in TTY.
False positives (re-appending an already-listed `.env`) waste
disk and corrupt audit; false negatives commit the key.

## Acceptance Criteria

Cargo test against the gitignore-check routine:

1. **`.env` already listed.** Compose a `.gitignore` with the
   exact line `.env`. Call the routine with `--yes`. Assert:
   - File is unchanged (byte-identical pre/post).
   - Routine reports outcome `Unchanged`.
2. **`.env` listed with leading slash.** Compose with `/.env`.
   Call the routine. Assert outcome `Unchanged` (leading slash
   is the same intent).
3. **`.env` missing, `.gitignore` exists.** Compose with one
   other entry, e.g. `target/`. Call with `--yes`. Assert:
   - File now has `.env` appended as a new line.
   - The pre-existing `target/` line is preserved.
   - Routine reports outcome `Appended`.
4. **`.gitignore` does not exist.** Empty repo, no
   `.gitignore`. Call with `--yes`. Assert:
   - `.gitignore` is created with `.env` as its only line.
   - Routine reports outcome `Created`.
5. **`.gitignore` is a directory.** Edge case — assert the
   routine returns `Err(InitError::GitignoreNotAFile)` rather
   than panicking or rewriting.

Trailing newline is normalised: appended line ends with `\n`
even if the existing file didn't.