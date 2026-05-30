---
id: TC-237
title: 'Config precedence chain: flag overrides env overrides config.toml overrides built-in default'
type: invariant
status: passing
validates:
  features:
  - FT-114
  adrs:
  - ADR-068
observes:
- stdout
phase: 4
runner: cargo-test
runner-args: tc_237_config_precedence_chain
runner-timeout: 30
last-run: 2026-05-30T07:52:25.933573475+00:00
last-run-duration: 0.7s
---

## Description

ADR-068's load-bearing invariant: a single deterministic
precedence chain (flag > env > config.toml > built-in
default) governs every config value. Without this property,
operators can't predict which override wins when multiple are
set; debugging becomes a guessing game.

## Acceptance Criteria

Cargo test over the `[driver].max_iter` key, which has a
built-in default of 6:

1. **Default only.** No config file, no env var, no flag.
   Assert resolved `max_iter == 6`.
2. **Config-toml wins over default.** Write
   `.dec/config.toml` with `[driver] max_iter = 8`. Assert
   resolved `max_iter == 8`.
3. **Env wins over config-toml.** Set `DEC_MAX_ITER=10` in
   the test process env (with the same config file from step 2).
   Assert resolved `max_iter == 10`.
4. **Flag wins over env.** Pass `--max-iter 12` to the
   resolver (with `DEC_MAX_ITER=10` still set, config still
   8). Assert resolved `max_iter == 12`.
5. **Flag absent, env absent, missing key in config-toml.**
   Write `.dec/config.toml` containing other keys but NOT
   `max_iter`. Assert resolved `max_iter == 6` (falls through
   to default for the missing key).

Repeat the chain check on two more keys with different value
types: `[sweep] default_format` (string) and `[sweep]
auto_retire_failing_graphs` (bool). The resolver must handle
all three TOML scalar types and respect the chain identically.