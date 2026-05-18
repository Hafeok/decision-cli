---
id: TC-002
title: dec_init_from_path_records_content_hash
type: exit-criteria
status: passing
validates:
  features: []
  adrs: []
phase: 1
runner: bash
runner-args: tests/scripts/tc-002-init-from-path.sh
runner-timeout: 60
last-run: 2026-05-18T19:02:15.368687079+00:00
last-run-duration: 0.3s
---

## Purpose

Validates the `--from <path>` branch of **ADR-006**: an equivalent store is produced from a local Turtle definition, with the source's **content hash and file path** recorded in the bootstrap session's PROV-O record (ADR-004).

Source: `decision-cli-slice-1-bounds.md` §11.2 exit-criteria #2.

## Given

- A fresh working directory with no `.dec/`.
- A valid `./streams/decision-cli-development.ttl` matching the template example in `decision-cli-slice-1-bounds.md` §3.2 (references `va:shipped-feature`, declares authorized goals `(ship land)`, etc.).

## When

```bash
dec init --from ./streams/decision-cli-development.ttl
```

## Then

1. The command exits 0.
2. `.dec/store/` exists with the same ValueStream/ValueAction shape produced by TC-001.
3. The bootstrap session record (`dec:session/init-001`) carries PROV-O triples recording:
   - The **file path** (`./streams/decision-cli-development.ttl`) as `prov:wasDerivedFrom` (or equivalent location property).
   - The **SHA-256 content hash** of the source bytes as a literal on the session.
   - The **ontology version** in effect at init time.
4. The hash recorded in the session matches the hash computed directly from the file on disk.

## Notes

- Section §3.7 / §11.1 enumerate the same provenance fields.
- TC-006 verifies that `dec status` surfaces the same source path and hash.