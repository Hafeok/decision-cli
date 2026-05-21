---
id: TC-042
title: function_length_within_adr_013_limits
type: invariant
status: passing
validates:
  features:
  - FT-051
  adrs:
  - ADR-013
phase: 1
runner: bash
runner-args: scripts/checks/function-length.sh
runner-timeout: 60
last-run: 2026-05-21T11:52:34.249757405+00:00
last-run-duration: 0.2s
---

## Purpose

Mechanical enforcement of **ADR-013 Rule 2 — Function Length Limit** for
Rust sources. Walks every `*.rs` file under `crates/*/src/` (excluding
`tests.rs` unit-test modules per the same exemption ADR-013 names for
`crates/*/tests/`) and counts statement lines inside each `fn` body
via awk-based brace-depth tracking.

Functions over 40 statement lines block CI (exit 1). Functions in the
30–40 band produce stdout `WARNING:` diagnostics but do not change the
exit code. A clean tree exits 0.

This TC has empty `validates.features` by design: per ADR-014,
code-quality rules are cross-cutting, validated against every feature
implicitly via `product verify --platform`.

## Given

- A working copy of decision-cli with `crates/*/src/` present.
- `bash` and `awk` available on `PATH` (no other dependencies).

## When

```bash
scripts/checks/function-length.sh
```

## Then

1. Exit 0 if no Rust function body under `crates/*/src/` (excluding
   `tests.rs` and `#[cfg(test)]` modules) exceeds `FN_LENGTH_HARD`
   statement lines (default 40). Functions in the `FN_LENGTH_WARN`–
   `FN_LENGTH_HARD` band (default 30–40), if any, are listed on stdout as
   advisory `WARNING:` diagnostics but do not change the exit code.
2. Exit 1 if at least one function body exceeds `FN_LENGTH_HARD` lines.
   Diagnostic lines on stdout name each offender with its file, line
   number, function name, and statement count.

## Notes

- Thresholds may be overridden via `FN_LENGTH_HARD` and `FN_LENGTH_WARN`
  environment variables — the same envelopes the Python companion
  (`function-length.py`) consumes so the contract stays uniform across
  implementation surfaces.
- Unit-test conventions are exempted in two forms: files literally named
  `tests.rs` (the inline test module convention) and any function body
  inside a `#[cfg(test)]`-annotated module. Integration tests under
  `crates/*/tests/` are exempted by the file selector itself.
- The brace-depth heuristic is intentionally simple over precise: raw
  strings and macro invocations with braces may produce minor miscounts.
  The remedy for any borderline false positive is to split the function,
  which is what ADR-013 Rule 2 prescribes anyway.
- Earlier revisions of this TC defined a tri-state exit code (0=clean /
  1=hard / 2=warn). The warn tier was dropped when ADR-013 was amended:
  product-cli's test runner treats anything other than exit 0/1 as
  `unrunnable`, so the warn-band signal moved to stdout diagnostics. The
  hygiene work of shrinking warn-band offenders is tracked in a separate
  feature_spec, not by gating this TC.

## Formal specification

⟦Σ:Types⟧{
  Function ≜ ⟨file:Path, line:ℕ, name:Ident, body_stmts:ℕ⟩
  RustSource ≜ {f:File | f.path matches "crates/*/src/**/*.rs"
                       ∧ ¬(f.path matches "**/tests.rs")
                       ∧ ¬(f.path matches "**/tests/**")
                       ∧ ¬(f.path matches "**/benches/**")}
  TopLevelFn ≜ {fn:Function | fn.file ∈ RustSource
                            ∧ ¬inside_cfg_test(fn)}
  HardLimit ≜ ℕ where HardLimit ≜ env(FN_LENGTH_HARD, default=40)
  WarnLimit ≜ ℕ where WarnLimit ≜ env(FN_LENGTH_WARN, default=30)
}

⟦Γ:Invariants⟧{
  ∀fn:TopLevelFn: fn.body_stmts ≤ HardLimit
  ∀fn:TopLevelFn: fn.body_stmts > WarnLimit ⇒ produces_advisory_warning(fn)
}

⟦Ε⟧⟨δ≜0.90;φ≜95;τ≜◊⁺⟩