---
id: TC-024
title: prov_o_lineage_in_writer_and_init
type: scenario
status: passing
validates:
  features: []
  adrs:
  - ADR-004
phase: 1
runner: bash
runner-args: scripts/checks/prov-o-lineage.sh
runner-timeout: 60
last-run: 2026-05-21T15:20:45.039212350+00:00
last-run-duration: 0.0s
---

## Purpose

Mechanical enforcement of **ADR-004 PROV-O for Events and Sessions**.
Asserts that the writer and init pipelines still reference the canonical
PROV-O predicates `prov:wasGeneratedBy`, `prov:wasDerivedFrom`, and
`prov:atTime` — the lineage scaffolding every Session/Event/Dispatch
artifact relies on (TC-013 / TC-015 are downstream of this).

## Given

- A working copy of decision-cli with `crates/oxi-events/src/writer/`
  and `crates/decision-cli/src/init/` present.
- `bash` and `grep` available on `PATH`.

## When

```bash
scripts/checks/prov-o-lineage.sh
```

## Then

1. Exit 0 if all three canonical PROV-O predicates appear under
   `crates/oxi-events/src/writer/` and `crates/decision-cli/src/init/`.
2. Exit 1 otherwise; diagnostic lines on stdout name the missing
   predicate.

## Formal Specification

⟦Σ:Types⟧{
  ProvPredicate ≜ wasGeneratedBy | wasDerivedFrom | atTime
  Module ≜ crates/oxi-events/src/writer | crates/decision-cli/src/init
}

⟦Γ:Invariants⟧{
  ∀ p:ProvPredicate, m:Module: references(m, p)
}