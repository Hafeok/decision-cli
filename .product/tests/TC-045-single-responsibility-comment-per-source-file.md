---
id: TC-045
title: single_responsibility_comment_per_source_file
type: invariant
status: passing
validates:
  features: []
  adrs:
  - ADR-013
phase: 1
runner: bash
runner-args: scripts/checks/single-responsibility.sh
runner-timeout: 60
last-run: 2026-05-22T10:27:09.181909747+00:00
last-run-duration: 2.0s
---

## Purpose

Mechanical enforcement of **ADR-013 Rule 4 — Single Responsibility
Naming Contract**. Every first-party source file must begin with a
one-sentence responsibility comment. The sentence must not contain the
word `and` (with surrounding spaces) — if it does, the file has two
responsibilities and must be split.

- Rust (`crates/*/src/**/*.rs`): first non-shebang line must start with
  `//! `.
- Python (`workers/*/**/*.py`): first non-shebang line must start with
  a triple-quoted docstring (`"""`).

This TC has empty `validates.features` by design: per ADR-014, the
single-responsibility rule is cross-cutting.

## Given

- A working copy of decision-cli with `crates/*/src/` and
  `workers/*/src/` populated.
- `bash`, `awk`, `grep`, and `sed` available on `PATH`.

## When

```bash
scripts/checks/single-responsibility.sh
```

## Then

1. Exit 0 if every first-party Rust file's first non-shebang line
   starts with `//! ` AND every first-party Python file's first
   non-shebang line starts with `"""`, AND neither sentence contains
   the substring ` and `.
2. Exit 1 if any file is missing the responsibility comment OR contains
   ` and ` in the sentence. Diagnostic lines on stdout name the file
   and the offending line.

## Notes

- `tests.rs` files (the inline unit-test module convention) are
  exempted, matching ADR-013's general "test files are exempt" carveout.
- The script does not enforce ADR-013's deferred constraint on
  `crates/oxi-events` doc-comment vocabulary (no DDD-specific terms in
  the substrate's responsibility comments) — that is scoped out of
  FT-014 and lands as a separate feature when the forbidden-words check
  is authored.
- The substring matched is ` and ` with bounding word breaks (start of
  line / end of line / spaces). Compound words like `command-and-control`
  or `bandwidth` do not match.

## Formal specification

⟦Σ:Types⟧{
  SourceFile ≜ ⟨path:Path, first_line:String, lang:Lang⟩
  Lang ≜ Rust | Python
  RustFirstLineWellFormed(f) ≜ f.first_line starts with "//! "
  PyFirstLineWellFormed(f) ≜ f.first_line starts with "\"\"\""
  HasDualResponsibility(f) ≜ f.first_line contains " and "
  FirstPartySource ≜ {f:SourceFile |
    (f.path matches "crates/*/src/**/*.rs" ∧ ¬(f.path matches "**/tests.rs"))
    ∨ (f.path matches "workers/**/*.py" ∧ ¬(f.path matches "**/tests/**"))
  }
}

⟦Γ:Invariants⟧{
  ∀f:FirstPartySource: well_formed(f)
  ∀f:FirstPartySource: ¬HasDualResponsibility(f)
}

⟦Ε⟧⟨δ≜0.90;φ≜100;τ≜◊⁺⟩