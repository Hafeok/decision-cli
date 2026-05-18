---
id: ADR-007
title: Embedded base ontology and bundled templates as slice 1 distribution model
status: accepted
features: []
supersedes: []
superseded-by: []
domains: []
scope: feature-specific
content-hash: sha256:baf2692f3e2ed974ff20a68f3305d0926168c3a86d4d368a9488e81d6602468b
---

## Context

decision-cli needs:

- A **base ontology** declaring `dec:ValueStream`, `dec:ValueAction`, `dec:Goal`, `dec:Session`, `dec:Dispatch`, `dec:Event` with SHACL shapes constraining their required fields.
- A **bundled library of canonical ValueAction definitions** (at minimum `va:shipped-feature` for slice 1).
- A **bundled library of ValueStream templates** (at minimum `engineering-development`).

These can be distributed in several ways:

1. **Embedded static assets.** Compiled into the binary via `include_bytes!`. Versioned with the binary; no external dependencies.
2. **Filesystem files.** Shipped alongside the binary in a known location; loaded at runtime.
3. **Network-fetched.** Pulled from a registry on first use.

For slice 1, the constraint is reproducibility: the same binary version must always parse the same ontology and resolve `va:shipped-feature` to the same definition. A network fetch introduces a moving target; a filesystem layout invites users to "fix" things in place and lose reproducibility.

See `decision-cli-slice-1-bounds.md` §3.1, §5.3, §6.1.

## Decision

For slice 1, the base ontology and the bundled definition library are **embedded as static assets in the binary** (`include_bytes!`-style).

- **Versioned with the binary.** Every `dec init` records the ontology version it used in the bootstrap session.
- **Validated at first use.** The embedded bytes are parsed lazily and the parse is treated as an invariant: a malformed embedded asset is a build-time bug, not a runtime concern.
- **Content-hashed.** The ontology and each bundled definition have stable content hashes recorded in PROV-O lineage.

Slice 1's bundled set: at minimum `va:shipped-feature` (ValueAction), `engineering-development` (ValueStream template). Additional definitions added as later slices need them.

Network fetch and a registry/catalog server are explicitly **deferred** to later slices; slice 1 supports only bundled templates and local file paths (see ADR-006).

## Consequences

**Positive:**

- The binary is self-contained: no filesystem layout assumptions, no network requirements at init time.
- Reproducibility is automatic: same binary version, same ontology, same bundled definitions.
- Distribution is one artifact (the binary), not a binary plus a definitions tarball.
- Validating the embedded ontology at first use catches build-time corruption immediately.

**Negative / accepted costs:**

- Updating the bundled definitions requires a binary release.
- Users wanting unbundled definitions must use `--from <path>` (acceptable: this is the slice 1 contract).
- A future registry/catalog server has not been designed; when it lands, the embedding remains as the offline fallback.

## Status

Accepted. Governs FT-006 (embedded base ontology + SHACL shapes) and FT-007 (bundled ValueAction + ValueStream template library).
