---
id: FT-101
title: 'decision-cli: CapabilityReference, OntologyDescription, ExemplarGraph artifact types'
phase: 3
status: planned
depends-on: []
adrs:
- ADR-066
- ADR-029
- ADR-028
tests:
- TC-165
- TC-166
- TC-167
domains: []
domains-acknowledged: {}
---

## Description

Three new dec artifact types that serve as the **substrate for [ADR-066](ADR-066)'s bundle-completeness principle**. The verify-graph-author bundle assembler ([FT-102](FT-102)) reads these via SPARQL `CONSTRUCT` queries rather than carrying hardcoded literals, so the system's view of "what dec looks like" is queryable graph state rather than assembler source code.

- `dec:CapabilityReference` — structured reference for every `dec` subcommand the worker may invoke from a `shell-command` step (verb, flags, exit codes, observable side effects). Stored at `.dec/catalog/capabilities/CR-NNN.ttl`.
- `dec:OntologyDescription` — the dec namespace plus the typed classes and canonical predicate set per class. Stored at `.dec/catalog/ontology/OD-NNN.ttl`. There is normally exactly one active `OntologyDescription` per stream; versioning is via supersession edges, not parallel multiplicity.
- `dec:ExemplarGraph` — a curated, validated `dec:VerificationGraph` (or reference to one) that has been tagged as a known-good pattern for an env's `safety_class`. Stored at `.dec/catalog/exemplars/EX-NNN.ttl`. Three to five per env type — enough to pattern-match against, not so many the worker drowns.

This slice ships the **types, the SHACL shapes, and the CLI verbs to author them**. The bundle assembler that *reads* them is [FT-102](FT-102); the chokepoint validator that enforces "any reference outside the bundle is a gap" is also [FT-102](FT-102). Splitting the substrate from its consumer follows the same pattern [FT-097](FT-097) / [FT-098](FT-098) used (artifact types first, executor second).

One subcommand → one slice — this slice carries three CLI verbs (`dec catalog capability new`, `dec catalog ontology new`, `dec catalog exemplar new`) and their MCP twins per [ADR-029](ADR-029); they share enough structure that one slice is the right grain. Each type's `show` / `list` verbs are folded in here, not split out.

## Functional Specification

### Inputs

#### `dec catalog capability new`

```
dec catalog capability new <CR-NNN> \
    --command <name> \
    --version <semver>           # the dec release the reference describes
    [--from-file <path>]          # JSON file with the structured surface
    [--from-stdin]
```

The reference body is a JSON document (validated against a Pydantic schema and persisted as a Turtle blob via `dec:capabilityBody`):

```json
{
  "command": "dec verify graph new",
  "synopsis": "Author a new dec:VerificationGraph artifact",
  "flags": [
    {"name": "--verifies", "value_kind": "string", "required": true, "description": "Feature or TC id the graph verifies"},
    {"name": "--environment", "value_kind": "string", "required": true, "description": "Env id"},
    {"name": "--id", "value_kind": "string", "required": false, "description": "Optional explicit VG-NNN id; auto-minted if absent"}
  ],
  "exit_codes": [
    {"code": 0, "meaning": "graph authored and persisted"},
    {"code": 1, "meaning": "validation or persistence error"}
  ],
  "observable_effects": [
    {"kind": "file_written", "path_pattern": ".dec/verify/graph/VG-*.ttl"},
    {"kind": "event_emitted", "event_type": "dec:VerificationGraphCreated"}
  ],
  "stdout_shape": {"format": "text", "fields": ["graph_id", "path"]}
}
```

#### `dec catalog ontology new`

```
dec catalog ontology new <OD-NNN> \
    --namespace <iri>             # canonical dec namespace
    --version <semver>            # the dec release the description describes
    [--from-file <path>]          # JSON file with the vocabulary surface
```

```json
{
  "namespace": "https://decision-cli.dev/ns#",
  "prefix": "dec",
  "classes": [
    {"local_name": "VerificationGraph", "iri": "https://decision-cli.dev/ns#VerificationGraph",
     "predicates": [
       {"local_name": "verifies", "range": "dec:Feature | dec:TC"},
       {"local_name": "environment", "range": "dec:VerificationEnvironment"},
       {"local_name": "steps", "range": "rdf:List of dec:VerificationStep"}
     ]},
    ...
  ],
  "ranges_summary": "see /docs/ddd/Decision-Driven_Design__Entity_Reference.md"
}
```

#### `dec catalog exemplar new`

```
dec catalog exemplar new <EX-NNN> \
    --graph <VG-NNN>              # the existing VG being promoted to exemplar
    --safety-class <class>        # which env safety_class this exemplar applies to
    --pattern-name <slug>         # short label e.g. "store-init-then-sparql"
    --rationale <text>            # why this is exemplary (≥ 40 chars)
```

The exemplar is a **reference** to an existing VG plus metadata; the VG's content is not duplicated. Promotion to exemplar requires that the referenced VG has at least one `VerificationGraphResult` with `verdict = approved` (defense against tagging untested graphs as templates).

#### MCP twins

Each verb has a paired MCP tool (`dec_catalog_capability_new`, `dec_catalog_ontology_new`, `dec_catalog_exemplar_new`) with identical semantics. Per [ADR-029](ADR-029) single-handler discipline.

### Outputs

- One `.ttl` file per artifact under the relevant `.dec/catalog/<type>/` directory.
- Standard CLI confirmation block (id, path, content-hash).
- For `exemplar new`: extra confirmation line naming the referenced VG and its most-recent verdict, so the operator can sanity-check the promotion.

### State

- New on-disk directories: `.dec/catalog/capabilities/`, `.dec/catalog/ontology/`, `.dec/catalog/exemplars/`.
- New ontology types projected into the `dec:` namespace (extends [FT-006](FT-006)'s embedded ontology bundle).
- New SHACL shapes (`dec:CapabilityReferenceShape`, `dec:OntologyDescriptionShape`, `dec:ExemplarGraphShape`) shipped in the same bundle path [FT-036](FT-036) ships its shapes from.
- Each artifact carries dual provenance per [ADR-038](ADR-038): mechanical (`prov:wasGeneratedBy`, `dcterms:created`, `prov:wasAttributedTo`) plus motivational (`dec:authoredFor` linking to whatever bundle-assembler / dispatch / human gesture motivated the authoring).

### Behaviour

#### Shape — `CapabilityReference`

```turtle
<https://decision-cli.dev/ns/cr/CR-001>
    a dec:CapabilityReference ;
    dec:command "dec verify graph new" ;
    dec:capabilityVersion "0.3.0" ;
    dec:capabilityBody """<JSON literal escaped here>""" ;
    dec:supersedes <cr/CR-000> ;          # optional — earlier reference for this command
    prov:wasGeneratedBy <activity/...> ;
    dcterms:created "..."^^xsd:dateTime .
```

Invariants:

- `dec:command` is unique among the **non-superseded** set. The handler refuses a `new` whose command is already covered by a non-superseded reference; the operator must `dec catalog capability supersede` first.
- `dec:capabilityBody` is validated against `CapabilityBodyShape` (a SHACL+Pydantic translation of the JSON schema in §Inputs) at write time. A malformed body is `Error::SchemaViolation` from `StreamWriter`, not silently accepted.
- The reference is **versioned**: `dec:capabilityVersion` is a semver string matching the `dec` release the reference describes. The bundle assembler selects the reference whose version matches the running `dec` binary (resolved at dispatch time).

#### Shape — `OntologyDescription`

```turtle
<https://decision-cli.dev/ns/od/OD-001>
    a dec:OntologyDescription ;
    dec:namespace "https://decision-cli.dev/ns#" ;
    dec:prefix "dec" ;
    dec:ontologyVersion "0.3.0" ;
    dec:ontologyBody """<JSON literal escaped here>""" ;
    dec:supersedes <od/OD-000> ;
    prov:wasGeneratedBy <activity/...> ;
    dcterms:created "..."^^xsd:dateTime .
```

Invariants:

- At most **one non-superseded** `OntologyDescription` per stream. The handler enforces this with a SPARQL constraint: a `new` that would create a parallel active description is rejected with `Error::SchemaViolation { detail: "supersede the existing active description first" }`.
- The body schema declares every class the assembler may expose to a worker. A predicate referenced in the body but absent from the SHACL shapes shipped by [FT-006](FT-006) is a write-time violation — keeps the ontology description and the actual enforcement in lockstep.

#### Shape — `ExemplarGraph`

```turtle
<https://decision-cli.dev/ns/ex/EX-001>
    a dec:ExemplarGraph ;
    dec:exemplarOf <graph/VG-042> ;
    dec:appliesToSafetyClass "isolated" ;
    dec:patternName "store-init-then-sparql" ;
    dec:rationale "Initialises a fresh oxigraph store, seeds three triples from inline Turtle, then runs a single sparql-assertion validating the seed succeeded. Canonical template for any verification that needs a known initial store state." ;
    dec:basedOnApprovedResult <result/VGR-099> ;
    prov:wasGeneratedBy <activity/...> ;
    dcterms:created "..."^^xsd:dateTime .
```

Invariants:

- `dec:exemplarOf` must resolve to an existing `dec:VerificationGraph`. SHACL `sh:class` check at write time.
- `dec:basedOnApprovedResult` must resolve to a `dec:VerificationGraphResult` whose `dec:verdict = "approved"` AND `dec:resultOf = <the same VG>`. SHACL `sh:sparql` constraint at write time. **An exemplar that has never passed is not exemplary** — this is the rule the user surfaced.
- `dec:appliesToSafetyClass` is one of the controlled vocabulary values from [ADR-028](ADR-028): `isolated`, `shared-non-destructive`, `production-readonly`.
- `dec:rationale` ≥ 40 chars (slightly stricter than [ADR-018](ADR-018)'s 20-char rule because an exemplar's rationale teaches the LLM what makes the pattern good — short rationales are a code smell).

#### Reading surface (used by [FT-102](FT-102) and `dec catalog * show`/`list`)

Each type has a `list` verb (filter by version / safety-class / supersession state) and a `show` verb (full Turtle render + parsed JSON body for `CapabilityReference` and `OntologyDescription`). The `list` verb defaults to showing the **active** set (non-superseded); `--include-superseded` reveals the history. These verbs are how [FT-102](FT-102)'s bundle assembler queries the catalog — no separate read path.

#### Supersession

Each type carries a `supersede` verb:

```
dec catalog capability supersede <CR-OLD> --by <CR-NEW>
dec catalog ontology   supersede <OD-OLD> --by <OD-NEW>
dec catalog exemplar   supersede <EX-OLD> --by <EX-NEW>   # rarely needed
```

The verb writes the `dec:supersedes` predicate on the new and `dec:supersededBy` (inverse) on the old, via `StreamWriter`. The active-set queries in `list` honour the supersession edges.

### Invariants

- Every catalog write goes through `StreamWriter` — SHACL validated, content-hashed, dual provenance attached.
- Catalog artifacts are **immutable once written**. Updates are via supersession, not in-place edit. This is the [ADR-002](ADR-002) graph-as-state pattern applied to dec's self-description.
- The catalog is **separate from the verification artifacts**: `.dec/catalog/` is dec's self-description; `.dec/verify/` is what verification produces. A worker that confuses the two paths (a `shell-command` step writing to `.dec/catalog/` from a verification run) is a SHACL violation — `dec:VerificationStep`'s file-write targets must not start with `.dec/catalog/`.
- `CapabilityReference.dec:command` uniqueness is enforced **across the active set only** (non-superseded). Supersession is the only path to evolve a command's reference.
- An `ExemplarGraph` whose underlying VG is deleted is **not** auto-deleted — it becomes an `OrphanedExemplar` (a sub-class warning surfaced by `dec catalog exemplar list --orphans`). Cleanup is operator-driven; auto-deletion would risk eating exemplars during routine VG housekeeping.

### Error handling

- `dec catalog capability new` with a body that fails Pydantic validation → `Error::InvalidInput { field, reason }`; exit 1.
- Duplicate active `dec:command` → `Error::DuplicateActive { existing }`; exit 1 with a hint to `supersede` first.
- `dec catalog exemplar new` whose target VG has no approved result → `Error::ExemplarNotProven { vg, latest_verdict }`; exit 1.
- Supersession of an ID that doesn't exist → `Error::ArtifactNotFound`; exit 1.
- Cycle in supersession edges (`A supersedes B`, `B supersedes A`) → `Error::SupersessionCycle`; exit 1. SHACL detects this at write time.

### Boundaries

- **In scope.** The three artifact types (Rust struct + Turtle shape + SHACL shape + JSON body schema), the embedded-ontology delta, the file-naming conventions (`CR-NNN`, `OD-NNN`, `EX-NNN`), the `new` / `show` / `list` / `supersede` CLI verbs and MCP twins for each type, supersession edge management, integration tests (one happy-path + one invariant-violation per type).
- **Out of scope.** The bundle assembler consuming these artifacts ([FT-102](FT-102)). The chokepoint validator that rejects out-of-bundle references ([FT-102](FT-102)). Automated capability-reference generation from the live `dec --help` output (could be a slice-3+ feature; v1 is manual authoring or hand-edited JSON). Multi-stream sharing of catalog artifacts (one stream = one catalog in v1). Dashboard rendering of the catalog (out of slice — surfaced through `list` for now).

## Out of scope

- Bundle assembler logic.
- Chokepoint validator.
- Auto-generation of CLI surface from `dec --help`.
- Cross-stream catalog sharing.
- Web/dashboard rendering.
- Migration of pre-FT-097 verification graphs into exemplars (a future cleanup feature can sweep, but is not required for FT-102 to function — the catalog starts empty and the bundle assembler ships with sane defaults until it isn't).
