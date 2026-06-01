---
id: TC-272
title: 'tool_safety: write_file to a secrets pattern returns structured tool_result error'
type: scenario
status: unimplemented
validates:
  features:
  - FT-124
  adrs:
  - ADR-071
phase: 4
observes:
- exit-code
runner: pytest
runner-args: workers/_shared/tests/test_tool_safety.py::test_secrets_patterns_blocked
runner-timeout: 30
---

## Description

[ADR-071](ADR-071) unconditionally refuses writes to known secrets patterns (`*.env`, `*.pem`, `*.key`, `*.pfx`, `*.p12`, `*.crt`, `secrets.{json,yaml,yml}`, `appsettings.production*`). This TC pins the behaviour of `is_write_blocked` against the patterns enumerated in the ADR.

The contract is: matches return True; near-misses (like `.env.example`) return False; reads of these paths are NOT blocked by this function (`is_write_blocked` is a write-side guard only).

## Acceptance Criteria

Pytest test at `workers/_shared/tests/test_tool_safety.py::test_secrets_patterns_blocked`.

For each of these paths, `is_write_blocked(path)` MUST return True:

- `.env`
- `config/.env`
- `secrets/database.env`
- `key.pem`
- `nested/path/cert.crt`
- `secrets.json`
- `config/secrets.yaml`
- `secrets.yml`
- `auth/cert.p12`
- `auth/identity.pfx`
- `config/appsettings.production.json`

For each of these paths, `is_write_blocked(path)` MUST return False:

- `README.md`
- `src/main.py`
- `.env.example` (note: `.env.example` does NOT match `*.env$`)
- `config/example.yaml`
- `secret-handling.md` (note: dash, not literal match)
- `production-config.toml`

The test asserts the False cases explicitly to prevent over-blocking regressions.
