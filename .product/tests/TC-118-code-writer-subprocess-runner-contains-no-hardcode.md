---
id: TC-118
title: code-writer subprocess runner contains no hardcoded model identifiers outside test fixtures
type: invariant
status: passing
validates:
  features: []
  adrs: []
phase: 1
runner: bash
runner-args: tests/scripts/tc-118-claude-env-no-hardcoded-models.sh
runner-timeout: 60
last-run: 2026-05-24T19:14:22.584356516+00:00
last-run-duration: 0.0s
---

## Description

Structural invariant for the code-writer worker (FT-066): every model
identifier passed to `claude -p` flows through `payload.model_id` —
which the Rust dispatcher pins from the resolved `dec:Capability` —
and never originates from a string literal in the worker's source.
Sibling to TC-113 (the same property for the verifier worker).

## Given

- The code-writer worker source tree at `workers/code-writer/src/`.
- A small allow-list of files where model identifiers may legitimately
  appear as literals: test fixtures under `workers/code-writer/tests/`,
  this TC's runner script, and documentation files.

## When

```bash
tests/scripts/tc-118-claude-env-no-hardcoded-models.sh
```

The script greps `workers/code-writer/src/` for known model-id patterns
(`claude-opus-*`, `claude-sonnet-*`, `claude-haiku-*`, `qwen3-*`,
`devstral-*`, plus the literal substring `ANTHROPIC_MODEL = "`) and
fails on any hit outside the allow-list.

## Then

- The chained exit code is 0.
- Stdout reports the scanned file count and any allow-listed matches.
- Stderr is empty.
- A non-empty match in source code (outside the allow-list) exits 1
  with the file:line:match diagnostic.

## Notes

This is the cross-feature application of [ADR-047](ADR-047)'s
capability-tag binding rule (workers never see model names except as
they appear on the bundle they were dispatched with). TC-113 enforces
the same property on the verifier worker; TC-118 covers code-writer.

## Formal specification

⟦Σ:Types⟧{
  ModelLiteral ≜ ⟨file:Path, line:ℕ, value:String⟩
  CodeWriterSrc ≜ {f:File | f.path matches "workers/code-writer/src/**/*.py"
                          ∧ ¬(f.path matches "**/tests/**")}
  AllowList ≜ {p:Path | p matches "workers/code-writer/tests/**"
                      ∨ p matches "tests/scripts/tc-118-*.sh"
                      ∨ p matches "**/*.md"}
  KnownModelPattern ≜ /claude-(opus|sonnet|haiku)-\d+(\.\d+)*|qwen3-[a-z0-9-]+|devstral-[a-z0-9-]+/
}

⟦Γ:Invariants⟧{
  ∀l:ModelLiteral: l.file ∈ CodeWriterSrc
                 ∧ l.value matches KnownModelPattern
                 ⇒ l.file ∈ AllowList
  ∀call:SubprocessSpawn(claude_p):
    env_var_for(call, "ANTHROPIC_MODEL") originates_from payload.model_id
}

⟦Ε⟧⟨δ≜0.90;φ≜95;τ≜◊⁺⟩
