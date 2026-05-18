---
id: ADR-006
title: Definition documents over raw strings for value stream init
status: accepted
features: []
supersedes: []
superseded-by: []
domains: []
scope: feature-specific
content-hash: sha256:19e921a9c05be98ccd49e52ec7ab2672ceda368c8645c98a1e6d22366cf20593
---

## Context

The v0.1 sketch had `dec init` take two strings: `--stream <name>` and `--value-action <name>`. This shape is wrong for the framework's claims:

- Strings carry **no schema**: missing fields, typos, and divergent interpretations are silent.
- Strings carry **no validation**: there is no way to enforce that two instances declaring `shipped-feature` mean the same thing.
- Strings carry **no provenance**: nothing records where the meaning came from or when it was decided.
- Strings carry **no reusability**: every team re-invents the meaning of common value actions.

The DDD framing explicitly aims to prevent this kind of drift. Letting it in at the bootstrap moment — the most load-bearing decision an instance makes — would undermine every subsequent claim about graph integrity.

See `decision-cli-slice-1-bounds.md` §3.1, §3.2, §3.3.

## Decision

`dec init` **does not accept raw strings** for stream or value-action identity. It accepts only a **reference to a schema-validated definition document**, in one of two forms:

```bash
dec init --template <bundled-template-name>     # resolves against bundled set
dec init --from <path-to-definition.ttl>        # resolves against a local file
```

The init flow is the five-step pipeline in §3.3:

1. **Parse** the definition (Turtle or JSON-LD).
2. **SHACL-validate** against the embedded base ontology.
3. **Resolve** the referenced `dec:terminalValueAction` URI against the bundled definition library.
4. **Cross-validate** that authorized goals intersect the ValueAction's compatible-goals set.
5. **Persist** the validated artifacts and **record** a bootstrap session with full PROV-O provenance.

If any step fails, **no state is written** to the orchestration store. The error names the failing field/URI/goal and the relevant constraint set.

The CLI explicitly **refuses to default-guess** value stream identity. There is no `dec init` with no arguments.

## Consequences

**Positive:**

- Two instances referencing the same canonical ValueAction URI are **provably** aiming at the same thing.
- The bootstrap is auditable: `dec session show init-001` exposes the source document, hash, validation outcome, and ontology version.
- The definition document is version-controllable (committed as a `.ttl` in the repo); the stream's identity is itself a reviewable artifact.
- Failure modes are clear and structured: missing fields, unknown URIs, unauthorized goals all produce actionable errors.

**Negative / accepted costs:**

- Higher friction for the first-time user: they must point at a template or write a definition file.
- The bundled definition library (FT-007) must be curated and maintained as part of the binary distribution.
- Slice 1 cannot fetch definitions from URLs (deferred per §6.2); only bundled templates and local file paths work.

**Explicit non-decisions:**

- This ADR does **not** decide the registry/catalog model for fetching definitions over the network. That is slice-2+ work.
- This ADR does **not** decide ValueStream composition (extending a base template with overrides) — also deferred.

## Status

Accepted. Governs FT-008 (init validation logic) directly, and constrains the CLI shape in FT-012 (no raw-string overrides).
