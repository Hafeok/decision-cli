---
id: TC-043
title: python_function_length_within_adr_013_limits
type: invariant
status: unrunnable
validates:
  features: []
  adrs:
  - ADR-013
phase: 1
runner: bash
runner-args: python3 scripts/checks/function-length.py
runner-timeout: 60
last-run: 2026-05-20T11:41:36.841111001+00:00
last-run-duration: 0.1s
failure-message: ""
---

## Purpose

Mechanical enforcement of **ADR-013 Rule 2 — Function Length Limit** for
Python sources. Walks every first-party `*.py` file under `workers/*/`
(excluding `tests/`, `__pycache__/`, and `.venv/`) and counts statement
nodes inside each `FunctionDef` / `AsyncFunctionDef` via the standard
library `ast` module.

Functions over 40 statement nodes block CI (exit 1). Functions in the
30–40 band produce warnings (exit 2). A clean tree exits 0.

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

1. Exit 0 if every Python function body under `workers/*/` (excluding
   tests and venv) has at most `FN_LENGTH_WARN` statement nodes
   (default 30).
2. Exit 2 if at least one function is in the 31–40 statement band and
   none exceeds the hard limit. Diagnostic lines on stdout name each
   warning function.
3. Exit 1 if at least one function exceeds `FN_LENGTH_HARD` statement
   nodes (default 40). Diagnostic lines on stdout name each offender
   with its file, line number, function name, and statement count.

## Notes

- Same envelopes (`FN_LENGTH_HARD`, `FN_LENGTH_WARN`) as the Rust
  companion (`function-length.sh`) — the contract is uniform across
  implementation surfaces (ADR-013 §"Rule scope").
- Counts statement *nodes* via `ast.walk` rather than statement *lines*.
  An expression spread across several physical lines counts as one
  statement, which is more accurate than the line-based Rust heuristic.
  The thresholds compensate for the difference.

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
  ∀fn:FirstPartyFn: fn.stmts > WarnLimit ⇒ produces_warning(fn)
}

⟦Ε⟧⟨δ≜0.95;φ≜100;τ≜◊⁺⟩