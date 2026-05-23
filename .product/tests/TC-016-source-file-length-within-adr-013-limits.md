---
id: TC-016
title: source_file_length_within_adr_013_limits
type: invariant
status: passing
validates:
  features: []
  adrs:
  - ADR-013
phase: 1
runner: bash
runner-args: scripts/checks/file-length.sh
runner-timeout: 60
last-run: 2026-05-23T18:00:16.213042199+00:00
last-run-duration: 0.3s
---

## Purpose

Mechanical enforcement of **ADR-013 Rule 1 — File Size Limit**. Asserts that
every first-party source file under `crates/*/src/` (Rust) and `workers/*/`
(Python, excluding `tests/`) is at most 400 lines (hard limit). Files in the
300–400 line band produce stdout `WARNING:` diagnostics but do not gate CI.
Files over 400 lines produce a hard failure (exit 1) — CI blocks the merge.

This TC has empty `validates.features` by design: per ADR-014, code-quality
rules are cross-cutting, validated against every feature implicitly via
`product verify --platform`.

## Given

- A working copy of the decision-cli repository checked out at any commit.
- `bash`, `awk`, `wc`, and `find` available on `PATH` (the only dependencies).

## When

```bash
scripts/checks/file-length.sh
```

## Then

1. Exit 0 if no first-party `*.rs` under `crates/*/src/` and no first-party
   `*.py` under `workers/*/` (excluding `tests/`) exceeds `FILE_LENGTH_HARD`
   lines (default 400). Files in the `FILE_LENGTH_WARN`–`FILE_LENGTH_HARD`
   band (default 300–400), if any, are listed on stdout as advisory
   `WARNING:` diagnostics but do not change the exit code.
2. Exit 1 if at least one such file exceeds `FILE_LENGTH_HARD` lines.
   Diagnostic lines on stdout name each offending file with its line count.

## Notes

- Thresholds may be overridden via `FILE_LENGTH_HARD` and `FILE_LENGTH_WARN`
  environment variables.
- TC-CQ-001 in the workspace narrative; TC-016 in the graph.
- This is the *first* mechanical rule shipped under the ADR-014 convention
  (FT-015). Subsequent rules (function length, module structure,
  single-responsibility doc comments) land as part of FT-014 and each adds
  its own TC pointing to the same parent ADR (ADR-013).
- Earlier revisions of this TC defined a tri-state exit code (0=clean /
  1=hard / 2=warn). The warn tier was dropped when ADR-013 was amended:
  product-cli's test runner treats anything other than exit 0/1 as
  `unrunnable`, so the warn-band signal moved to stdout diagnostics. The
  hygiene work of shrinking warn-band offenders is tracked in a separate
  feature_spec, not by gating this TC.

## Formal specification

⟦Σ:Types⟧{
  SourceFile ≜ ⟨path:Path, line_count:ℕ⟩
  FirstPartySource ≜ {f:SourceFile | f.path matches "crates/*/src/**/*.rs"
                                   ∨ (f.path matches "workers/**/*.py"
                                      ∧ ¬(f.path matches "**/tests/**")
                                      ∧ ¬(f.path matches "**/__pycache__/**"))}
  HardLimit ≜ ℕ where HardLimit ≜ env(FILE_LENGTH_HARD, default=400)
  WarnLimit ≜ ℕ where WarnLimit ≜ env(FILE_LENGTH_WARN, default=300)
}

⟦Γ:Invariants⟧{
  ∀f:FirstPartySource: f.line_count ≤ HardLimit
  ∀f:FirstPartySource: f.line_count > WarnLimit ⇒ produces_advisory_warning(f)
}

⟦Ε⟧⟨δ≜0.95;φ≜100;τ≜◊⁺⟩