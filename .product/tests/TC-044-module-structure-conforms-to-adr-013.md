---
id: TC-044
title: module_structure_conforms_to_adr_013
type: invariant
status: passing
validates:
  features: []
  adrs:
  - ADR-013
phase: 1
runner: bash
runner-args: scripts/checks/module-structure.sh
runner-timeout: 60
last-run: 2026-05-23T16:10:19.845721788+00:00
last-run-duration: 0.0s
---

## Purpose

Mechanical enforcement of **ADR-013 Rule 3 — Module Decomposition** plus
the 80-line cap on `crates/decision-cli/src/main.rs`. Asserts that the
canonical core modules ADR-013 names for each Rust crate are present
(either as `<name>.rs` or `<name>/mod.rs`), and that the binary entry
point stays dispatch-only.

The presence check is conservative: it asserts the canonical names that
are stable across both the current layout and the FT-018 vertical-slice
migration. Crates may legitimately grow additional modules (e.g.
`core/`, `features/`) without tripping this check — that is exactly what
ADR-016 calls for.

This TC has empty `validates.features` by design: per ADR-014, the
module-decomposition rule is cross-cutting.

## Given

- A working copy of decision-cli with `crates/oxi-events/src/` and
  `crates/decision-cli/src/` present.
- `bash` and `wc` available on `PATH`.

## When

```bash
scripts/checks/module-structure.sh
```

## Then

1. Exit 0 if every required canonical module is present in each crate
   AND `crates/decision-cli/src/main.rs` is at most 80 lines.
2. Exit 1 if a canonical module is missing from any crate's `src/`, or
   if `main.rs` exceeds the 80-line cap. Diagnostic lines on stdout name
   the missing module(s) and/or report the actual line count.

## Notes

- The canonical-module list for `oxi-events` is: `writer`, `subscription`,
  `replay`, `outbox` — the substrate's four load-bearing surfaces per
  ADR-013 §"Module Decomposition".
- The canonical-module list for `decision-cli` is restricted to the
  intersection that survives the FT-018 migration: `ontology`, `init`,
  `vocab`. After FT-018 lands, the list may expand to require `core/`
  and `features/` as well.
- `MAIN_RS_LINE_LIMIT` environment variable overrides the 80-line cap
  (default 80) — useful for ad-hoc relaxation during a refactor, but the
  intent is the cap is binding.

## Formal specification

⟦Σ:Types⟧{
  Module ≜ ⟨crate:Ident, name:Ident, exists:𝔹⟩
  CanonicalModules ≜ {
    oxi-events: {writer, subscription, replay, outbox},
    decision-cli: {ontology, init, vocab}
  }
  MainRsLines ≜ ℕ where MainRsLines ≜ lines("crates/decision-cli/src/main.rs")
  MainLimit ≜ ℕ where MainLimit ≜ env(MAIN_RS_LINE_LIMIT, default=80)
}

⟦Γ:Invariants⟧{
  ∀⟨c,n⟩ ∈ CanonicalModules: module_present(c, n)
  MainRsLines ≤ MainLimit
}

⟦Ε⟧⟨δ≜0.90;φ≜100;τ≜◊⁺⟩