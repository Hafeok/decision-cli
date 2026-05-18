---
id: TC-001
title: dec_init_template_persists_stream_and_action
type: exit-criteria
status: passing
validates:
  features: []
  adrs: []
phase: 1
runner: bash
runner-args: tests/scripts/tc-001-init-template.sh
runner-timeout: 60
last-run: 2026-05-18T19:02:15.368687079+00:00
last-run-duration: 0.2s
---

## Purpose

Validates the happy-path of the **ADR-006** init validation pipeline using the bundled `engineering-development` template (FT-007). After init, the `ValueStream` and `ValueAction` artifacts must both exist in the orchestration store (FT-009), both reachable via SPARQL, and both linked to the bootstrap session via PROV-O per **ADR-004**.

Source: `decision-cli-slice-1-bounds.md` §11.2 exit-criteria #1.

## Given

- A fresh working directory with no `.dec/`.
- A built `dec` binary with the embedded ontology (FT-006) and the `engineering-development` bundled template (FT-007).

## When

```bash
dec init --template engineering-development
```

## Then

1. The command exits 0.
2. `.dec/store/` exists and is a valid Oxigraph store.
3. The following SPARQL against the store returns exactly one row, with the expected bundled URI:
   ```sparql
   SELECT ?stream ?action WHERE {
     ?stream a dec:ValueStream ;
             dec:terminalValueAction ?action .
     ?action a dec:ValueAction .
   }
   ```
4. The bootstrap session `dec:session/init-001` exists and PROV-O-references both the `ValueStream` and the `ValueAction` (ADR-004): the chains `?stream prov:wasGeneratedBy <init-001>` and `?action prov:wasGeneratedBy <init-001>` (or equivalent PROV pattern) both resolve.

## Notes

- TC-002 covers the `--from <path>` equivalent.
- TC-015 covers the broader invariant that the bootstrap session is always reachable from the ValueStream.