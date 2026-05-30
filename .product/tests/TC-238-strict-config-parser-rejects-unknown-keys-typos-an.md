---
id: TC-238
title: Strict config parser rejects unknown keys, typos, and credential-shaped key names
type: scenario
status: passing
validates:
  features:
  - FT-114
  adrs:
  - ADR-068
observes:
- stderr
- exit-code
phase: 4
runner: cargo-test
runner-args: tc_238_strict_parser_rejects_invalid_config
runner-timeout: 30
last-run: 2026-05-30T16:41:58.446771378+00:00
last-run-duration: 0.5s
---

## Description

ADR-068 mandates strict parsing: silent typos and
forward-compat acceptance are explicitly rejected. The parser
fails CLI startup with a precise error naming the offending
file path, line number, key, and what was expected. Also
enforces the credential tripwire — keys that look like
credentials (`*secret*`, `*key*`, `*token*`, `*password*`)
are refused as a guard against accidental commit.

## Acceptance Criteria

Cargo test over `parse_config(&path)`:

1. **Unknown key error.** Write `.dec/config.toml` with
   `[driver] max_inter = 8` (note the typo `inter` not
   `iter`). Assert `parse_config` returns
   `Err(ConfigError::UnknownKey { key: "driver.max_inter",
   file: ".dec/config.toml", line: 2 })`. Assert the error's
   Display contains the substring "did you mean: max_iter?"
   (suggestion via Levenshtein distance).
2. **Type mismatch error.** Write `[driver] max_iter =
   "eight"` (string where int expected). Assert
   `Err(ConfigError::TypeMismatch { key: "driver.max_iter",
   expected: "integer", got: "string", file, line })`.
3. **Out-of-range error.** Write `[driver] max_iter = 0`
   (zero would short-circuit every drive immediately).
   Assert `Err(ConfigError::OutOfRange { key:
   "driver.max_iter", min: 1, got: 0, file, line })`.
4. **Credential tripwire.** Write a key like
   `[provider] scaleway_secret_key = "scw-..."`. Assert
   `Err(ConfigError::CredentialKeyForbidden { key, file,
   line })` and the error suggests "move to .env file."
5. **Valid config.** Write a config that mirrors ADR-068's
   initial inventory exactly. Assert `parse_config` returns
   `Ok(Config { ... })` with every field populated from the
   file (no falling through to defaults for keys present in
   the file).