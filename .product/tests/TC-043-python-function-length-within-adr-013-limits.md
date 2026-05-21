---
id: TC-043
title: python_function_length_within_adr_013_limits
type: invariant
status: passing
validates:
  features:
  - FT-051
  adrs:
  - ADR-013
phase: 1
runner: bash
runner-args: python3 scripts/checks/function-length.py
runner-timeout: 60
last-run: 2026-05-21T15:21:18.505743289+00:00
last-run-duration: 0.1s
---

## Purpose

Mechanical enforcement of **ADR-013 Rule 2 — Function Length Limit** for
Python sources. Walks every first-party `*.py` file under `workers/*/`
(excluding `tests/`, `__pycache__/`, and `.venv/`) and counts statement
nodes inside each `FunctionDef` / `AsyncFunctionDef` via the standard
library `ast` module.

Functions over 40 statement nodes block CI (exit 1). Functions in the
30–40 band produce stdout `WARNING:` diagnostics but do not change the
exit code. A clean tree exits 0.

This TC has empty `validates.features` by design: per ADR-014,
code-quality rules are cross-cutting.

## Given

- A working copy of decision-cli with `workers/code-writer/src/` present.
- `python3` (3.11+) available on `PATH` — no other dependencies.

## When

```bash
python3 scripts/checks/function-length.py
```

## Then

1. Exit 0 if no Python function body under `workers/*/` (excluding tests
   and venv) exceeds `FN_LENGTH_HARD` statement nodes (default 40).
   Functions in the `FN_LENGTH_WARN`–`FN_LENGTH_HARD` band (default
   30–40), if any, are listed on stdout as advisory `WARNING:`
   diagnostics but do not change the exit code.
2. Exit 1 if at least one function exceeds `FN_LENGTH_HARD` statement
   nodes. Diagnostic lines on stdout name each offender with its file,
   line number, function name, and statement count.

## Notes

- Same envelopes (`FN_LENGTH_HARD`, `FN_LENGTH_WARN`) as the Rust
  companion (`function-length.sh`) — the contract is uniform across
  implementation surfaces (ADR-013 §"Rule scope").
- Counts statement *nodes* via `ast.walk` rather than statement *lines*.
  An expression spread across several physical lines counts as one
  statement, which is more accurate than the line-based Rust heuristic.
  The thresholds compensate for the difference.
- Earlier revisions of this TC defined a tri-state exit code (0=clean /
  1=hard / 2=warn). The warn tier was dropped when ADR-013 was amended:
  product-cli's test runner treats anything other than exit 0/1 as
  `unrunnable`, so the warn-band signal moved to stdout diagnostics. The
  hygiene work of shrinking warn-band offenders is tracked in a separate
  feature_spec, not by gating this TC.

## Formal specification

⟦Σ:Types⟧{
  PyFunction ≜ ⟨file:Path, line:ℕ, name:Ident, stmts:ℕ⟩
  PySource ≜ {f:File | f.path matches "workers/**/*.py"
                     ∧ ¬(f.path matches "**/tests/**")
                     ∧ ¬(f.path matches "**/__pycache__/**")
                     ∧ ¬(f.path matches "**/.venv/**")}
  FirstPartyFn ≜ {fn:PyFunction | fn.file ∈ PySource}
  HardLimit ≜ ℕ where HardLimit ≜ env(FN_LENGTH_HARD, default=40)
  WarnLimit ≜ ℕ where WarnLimit ≜ env(FN_LENGTH_WARN, default=30)
}

⟦Γ:Invariants⟧{
  ∀fn:FirstPartyFn: fn.stmts ≤ HardLimit
  ∀fn:FirstPartyFn: fn.stmts > WarnLimit ⇒ produces_advisory_warning(fn)
}

⟦Ε⟧⟨δ≜0.95;φ≜100;τ≜◊⁺⟩