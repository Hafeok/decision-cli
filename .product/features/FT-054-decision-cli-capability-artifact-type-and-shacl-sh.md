---
id: FT-054
title: 'decision-cli: Capability artifact type and SHACL shape'
phase: 2
status: planned
depends-on: []
adrs:
- ADR-033
- ADR-036
tests:
- TC-100
domains:
- data-model
domains-acknowledged: {}
---

## Description

Introduce the `dec:Capability` artifact type — a versioned binding from a stable capability tag (e.g. `code-writer`, `standard-reasoning`, `deep-reasoning`) to a concrete `(endpoint, model_identifier, parameters)` triple plus declared model properties. The catalog of capabilities *is* a set of `dec:Capability` artifacts in the graph per [ADR-036](ADR-036); this feature lands the schema, the SHACL shape, and the ontology extension. Catalog content lives in [FT-058](FT-058).

The capability layer is the substrate for [ADR-033](ADR-033)'s claim that model selection is graph-resident policy, not worker code.

## Functional Specification

### Inputs

- The embedded base ontology ([FT-006](FT-006)) is extended with the `dec:Capability` class definition and its predicates.
- SHACL shape `dec:CapabilityShape` declared in the embedded shapes graph.

### Outputs

- New ontology terms (under the existing `dec:` namespace):
  - Class `dec:Capability`
  - `dec:capability_id` (xsd:string, required; one per artifact) — stable tag used by roles to reference this capability.
  - `dec:endpoint` (xsd:string, required; enum: `scaleway`, `anthropic`) — which external API the model is invoked through.
  - `dec:model_identifier` (xsd:string, required) — exact API model string (e.g. `qwen3-coder-30b-a3b-instruct`, `claude-opus-4-7`).
  - `dec:tier` (xsd:integer, optional; 0–3 when present, omitted for specialty capabilities) — escalation-ladder tier.
  - `dec:context_window` (xsd:integer, required; non-negative) — tokens.
  - `dec:max_output` (xsd:integer, required; non-negative) — tokens.
  - `dec:supports_vision` (xsd:boolean, required).
  - `dec:supports_tool_calling` (xsd:boolean, required).
  - `dec:cost_input_per_m` (xsd:decimal, required; non-negative) — EUR per 1M input tokens.
  - `dec:cost_output_per_m` (xsd:decimal, required; non-negative) — EUR per 1M output tokens.
  - `dec:configurable_effort` (xsd:boolean, optional; default false) — model accepts `reasoning_effort` parameter.
  - `dec:status` (xsd:string, required; enum: `active`, `preview`, `eol`, `candidate`).
  - `dec:version` (xsd:integer, required; ≥ 1).
  - `dec:supersedes` (optional; `dec:Capability` IRI) — links to prior version.
  - `dec:bootstrap_source` (xsd:string, optional) — content hash of seed YAML the artifact was bootstrapped from ([ADR-036](ADR-036) audit).
  - `dec:notes` (xsd:string, optional) — free-text for catalog maintainer comments (Scaleway API quirks, calibration notes).

### State

- Embedded ontology bytes ([FT-006](FT-006)) grow by the class + property declarations and the SHACL shape.
- `OntologyHandle::version()` ([FT-006](FT-006)) bumps; existing init flows refresh the ontology hash recorded in the orchestration store.

### Behaviour

1. Extend `crates/decision-cli/core/ontology/embedded.ttl` (or the slice's actual file) with the `dec:Capability` class declaration and predicate definitions.
2. Extend `embedded_shapes.ttl` with `dec:CapabilityShape`:
   - `sh:targetClass dec:Capability`.
   - `sh:property` for each required field with `sh:minCount 1` and `sh:datatype` constraint.
   - `sh:in` enum for `dec:endpoint` ∈ {`scaleway`, `anthropic`} and `dec:status` ∈ {`active`, `preview`, `eol`, `candidate`}.
   - `sh:minInclusive 0` on `dec:context_window`, `dec:max_output`, `dec:cost_input_per_m`, `dec:cost_output_per_m`.
   - `sh:minInclusive 1` on `dec:version`.
   - A `sh:sparql` constraint enforcing capability_id+version uniqueness within `active` status.
3. Add a Rust `core::ontology::capability` module exposing:
   - `pub struct Capability { id, endpoint, model_identifier, tier, context_window, max_output, supports_vision, supports_tool_calling, cost_input_per_m, cost_output_per_m, configurable_effort, status, version, supersedes, bootstrap_source, notes }`
   - `pub enum Endpoint { Scaleway, Anthropic }` with `as_str` / `try_from_str`.
   - `pub enum CapabilityStatus { Active, Preview, Eol, Candidate }`.
   - Deserialisation from `oxigraph::sparql::QuerySolution` rows.
4. Add a SPARQL CONSTRUCT helper `core::graph::capability::query_active_by_id(id)` returning the active capability with the given id, or `None`.

### Invariants

- A capability's `(capability_id, version)` is unique among `status = active` artifacts.
- `tier` may be `None` for specialty capabilities (e.g. `vision-general`, `embedding`); roles binding to them do not participate in tier-based escalation per [ADR-034](ADR-034).
- `configurable_effort = true` implies the dispatcher must compute `reasoning_effort` from `bundle.stakes` per [FT-063](FT-063).
- `endpoint = scaleway` implies the model is reachable via the OpenAI-compatible client from [FT-059](FT-059).
- A `Capability` artifact with `supports_tool_calling = false` cannot be the resolved capability for a role whose worker requires tool calling (implementer, verifier); this is enforced at dispatcher resolution time per [FT-061](FT-061), not at artifact write time (the catalog may carry tool-less specialty entries for future roles).

### Error handling

- SHACL violation on missing required field → graph write refused, error surfaces through `GraphWriter` ([FT-001](FT-001)).
- Enum violation on `endpoint` / `status` → SHACL violation; same path.
- A capability_id+version collision in active status → `sh:sparql` violation; the writer must `supersede` the prior version explicitly.

### Boundaries

- **In scope.** Ontology class, SHACL shape, Rust struct, SPARQL query helper.
- **Out of scope.** Catalog content (seed values) — [FT-058](FT-058). Role bindings — [FT-055](FT-055). Dispatcher resolution — [FT-061](FT-061). Workers consuming capabilities — workers never see capabilities (they see the resolved triple per [ADR-033](ADR-033)).

## Out of scope

- Auto-discovery of capabilities from provider APIs (manually curated for now).
- Per-region or per-availability-zone variants of a single capability (one endpoint string is enough until needed).
- Capability composition / inheritance (a future ADR would propose this if duplication becomes painful).
