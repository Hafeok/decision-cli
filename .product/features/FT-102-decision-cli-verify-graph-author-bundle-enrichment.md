---
id: FT-102
title: 'decision-cli: verify-graph-author bundle enrichment and dispatch-time completeness validator'
phase: 3
status: planned
depends-on:
- FT-101
- FT-048
- FT-049
adrs:
- ADR-066
- ADR-008
- ADR-029
- ADR-043
- ADR-023
tests:
- TC-168
- TC-169
- TC-170
- TC-171
domains: []
domains-acknowledged: {}
---

## Description

Two coupled changes that together implement [ADR-066](ADR-066) for the verify-graph-author role:

1. **Bundle enrichment** — extend the `VerifyGraphAuthorInput` Pydantic model ([FT-048](FT-048)) with the five fields ADR-066 mandates (`cli_surface`, `ontology_vocabulary`, `store_query_surface`, `env_capabilities`, `exemplar_graphs`). The bundle assembler in the orchestrator populates them by SPARQL-querying the catalog artifacts [FT-101](FT-101) ships. The worker's prompt is updated to consume them. The worker's body update is **the smallest** part of this slice — most of the work is the assembler.
2. **Dispatch-time completeness validator** — a chokepoint that runs *after* the worker returns and *before* the proposal is persisted. It walks every proposed step's referenced facts (commands, namespaces, env capabilities) and asserts each is present in the bundle that was sent. Out-of-bundle references → reject the proposal and emit a `dec:Feedback` with `class = "gap"` targeted at the bundle assembler, not at the worker. ADR-066 Rule 3 lives here.

The slice closes the loop the ADR opens. After it lands, regenerating verification graphs for FT-097..FT-100 (and every future feature) produces graphs whose steps reference only things the bundle declared — no hallucinated namespaces, no invented commands, no fake SPARQL targets.

One subcommand → one slice — there is no new subcommand here. The slice modifies an existing worker's bundle shape, the existing dispatch handler ([FT-049](FT-049)'s `dec verify graph generate`), and adds one validation pass. The slice is bigger than a single verb but narrower than two coupled deliverables would justify.

## Functional Specification

### Inputs

Two consumers shift shape in this slice:

#### 1. The bundle assembler (orchestrator side)

Lives in `features/verify_graph_generate/bundle.rs` today; gains five new fields populated from the catalog:

```rust
pub struct VerifyGraphAuthorInput {
    // existing fields per FT-048 ...
    pub cli_surface:         CliSurface,
    pub ontology_vocabulary: OntologyVocabulary,
    pub store_query_surface: StoreQuerySurface,
    pub env_capabilities:    EnvCapabilities,
    pub exemplar_graphs:     Vec<ExemplarGraphRecord>,
}
```

Each new field is populated by a SPARQL `CONSTRUCT` against the orchestration store:

| Field | Source | Selection rule |
|---|---|---|
| `cli_surface` | every active `dec:CapabilityReference` (non-superseded) | filter by `dec:capabilityVersion` matching the running `dec` binary version (resolved at dispatch time from `dec --version`) |
| `ontology_vocabulary` | the **one** active `dec:OntologyDescription` | invariant from [FT-101](FT-101): at most one non-superseded |
| `store_query_surface` | derived from the target env's `dec:envType` | constant table per env type (`ephemeral-tempdir` → `dec sparql query --store ./...`; `remote-http` → endpoint URL from `env.dec:endpoint`); maintained alongside the env type registry |
| `env_capabilities` | from the target env's optional `dec:concreteCapabilities` block (extends [FT-035](FT-035) — new optional predicate added here) | if absent, ship a default for the env type plus a `warning` on the bundle's metadata |
| `exemplar_graphs` | every `dec:ExemplarGraph` whose `dec:appliesToSafetyClass = env.dec:safetyClass` | sort by `dcterms:created DESC`, cap at 5 |

The bundle hash recomputes over the enriched payload.

#### 2. The worker (verify-graph-author)

The Pydantic model extends with matching fields; the prompt template gains five new sections (one per field). The prompt is explicit about the contract: *"You may only reference commands listed in `cli_surface`, namespaces and predicates listed in `ontology_vocabulary`, query targets listed in `store_query_surface`, and binaries/paths listed in `env_capabilities`. The orchestrator will reject any proposal that references something not in your bundle. Use `exemplar_graphs` as pattern templates."*

The worker package update is itself small (~50 lines) — the heavy lift is the assembler.

### Outputs

- Persisted `dec:VerificationGraph` artifacts whose steps reference only bundle-declared facts.
- Rejected proposals (when the validator finds violations) → `Error::ProposalReferencesOutOfBundle { violations: Vec<Violation> }`, plus one `dec:Feedback` with `class = "gap"` targeted at the bundle assembler with detail naming the missing catalog item.
- Bundle metadata records the catalog item content-hashes that were included — so a replay produces an identical bundle and the audit trail is `(catalog hashes, feature_spec hash, tc hashes) → bundle_hash → proposal hash → graph file hash`.

### State

- New on-disk: nothing — this slice writes through the existing `.dec/verify/graph/` path and emits `Feedback` through the existing `.dec/feedback/` path.
- Reads: every active catalog artifact under `.dec/catalog/`, the target env, the feature_spec, the TCs, the candidate graphs (existing inputs).
- The bundle assembler grows one query template per new field. These ship as `dec:QueryTemplate` artifacts ([ADR-043](ADR-043) pattern) so the assembler is itself declarative — changing what `cli_surface` selects is editing a query artifact, not editing assembler source.

### Behaviour

#### Bundle assembly

1. Resolve the running `dec` binary version (via `dec --version` once at orchestrator start; cached for the session).
2. Resolve the target env from the request.
3. For each of the five new fields, run its query template against the store. Failures (e.g. no active `OntologyDescription`) produce **structured warnings on the bundle**, not silent omissions — the worker sees the empty field and can decide to return a `Gap` proposal rather than fabricate.
4. Compute the new `bundle_hash` over the canonical serialisation of the enriched payload.
5. Subprocess-invoke the worker per the existing FT-048 dispatch path.

#### Worker dispatch (unchanged except for the prompt)

The worker's behaviour is otherwise identical: parse bundle → Claude call with structured output → return `GraphProposal`. The five new prompt sections shift the LLM toward bundle-grounded outputs; the worker does not validate the bundle itself (that's the chokepoint validator's job per ADR-066 Rule 3).

#### Dispatch-time completeness validation (the new chokepoint)

After the worker returns a `GraphProposal::New { steps }` and before the persistence path (FT-049 §step 9) writes the graph:

1. **Walk every step** and extract its referenced facts:
   - `shell-command` → the first token of `command` (the binary), plus any `dec <subcommand>` patterns matched against a regex.
   - `sparql-assertion` → every IRI prefix in the query (extracted via a lightweight SPARQL parser, falling back to regex for the `PREFIX foo: <bar>` declarations and `foo:` qualified names).
   - `http-request` → the URL's host and scheme.
   - `file-assertion` → the target path's prefix (compared against env-declared writable paths).
   - `capture` → bind sources (must reference declared environment variables for `env_var` kind, or a prior step's stdout for `prior_step_stdout`).
2. **For each referenced fact**, check membership in the bundle:
   - `shell-command` binary or `dec` subcommand → must be in `cli_surface.commands` (the binary) or `cli_surface.dec_subcommands` (the dec verb).
   - SPARQL IRI prefix → must be in `ontology_vocabulary.namespaces` (or be a standard W3C namespace from a whitelist: `rdf:`, `rdfs:`, `xsd:`, `owl:`, `prov:`, `dcterms:`).
   - HTTP host → must be in `env_capabilities.allowed_hosts` (declared on the env).
   - File path prefix → must be in `env_capabilities.writable_paths`.
   - Capture source → must be a declared `env_capabilities.environment_variables` name or a prior-step index.
3. **Build the violation set.** Each violation: `{ step_index, kind, referenced_thing, why_rejected }`.
4. **If non-empty**, refuse persistence; return `Error::ProposalReferencesOutOfBundle { violations }`. Emit one `dec:Feedback`:
   - `dec:class = "gap"`.
   - `dec:target` = the `dec:CapabilityReference` / `dec:OntologyDescription` / `dec:VerificationEnvironment` that was the natural place to register the missing fact. If multiple, emit one per natural target.
   - `dec:fromActivity` = the bundle assembly activity (not the worker activity — the gap is in the upstream, per ADR-066 Rule 3).
   - Body: the violation list rendered, plus a suggestion (`"add 'dec verify result inspect' to CR-NNN, then regenerate"`).
5. **If empty**, proceed to the existing FT-049 persistence path.

#### Worker-side hint loop (deliberate restraint)

The worker is **not** asked to retry on validator rejection. ADR-066 §Rule 3 makes the gap the upstream's problem — the operator extends the relevant catalog artifact and `dec verify graph generate` is re-run. Adding an auto-retry would smuggle a remedy into the dispatch loop that ADR-066 deliberately keeps external. (This is the same stance FT-049 takes on `ProposalStale` — re-run, don't auto-retry.)

#### Env capability extension

The `dec:VerificationEnvironment` shape ([FT-035](FT-035)) gains an optional `dec:concreteCapabilities` block:

```turtle
<env:ENV-001>
    a dec:VerificationEnvironment ;
    dec:envType "ephemeral-tempdir" ;
    dec:allowedOps ( "shell" "filesystem" "sparql-local" ) ;
    dec:concreteCapabilities [
        dec:binariesOnPath        ( "dec" "bash" "jq" "sparql" ) ;
        dec:writablePaths         ( "$DEC_VERIFY_TMP" "./" ) ;
        dec:allowedHosts          ( ) ;          # http only when envType = remote-http
        dec:environmentVariables  ( "DEC_VERIFY_TMP" "PATH" "HOME" ) ;
        dec:preSeededArtifacts    ( )
    ] .
```

The block is **optional** (SHACL `sh:maxCount 1`) — envs that predate this slice continue to validate; the assembler ships a default-per-env-type table for them and surfaces a `warning` on the bundle so operators see which envs need a refresh.

### Invariants

- The bundle assembler **never** hardcodes catalog content — every value in the five new fields comes from a SPARQL query result against `.dec/catalog/`. The query templates themselves are `dec:QueryTemplate` artifacts ([ADR-043](ADR-043)). Asserted by a code-fitness test: grep'ing assembler source for the `dec:` literal namespace must return zero hits inside the field-population functions.
- The chokepoint validator **never** consults the catalog directly — it works only against the bundle that was sent to the worker. This preserves replay determinism: re-running the validator over the same `(bundle, proposal)` pair must yield the same verdict regardless of catalog state at re-run time.
- An empty field on the bundle (e.g. zero exemplars matching the safety class) is **legal**; it produces a `warning` on the bundle but does not refuse dispatch. The worker decides whether to return `Gap` or proceed; the validator does not treat empty fields as a violation.
- Validation rejection **does not retry** the worker. One bundle → one proposal → accept or reject. ADR-066 Rule 3.
- The five new fields' content-hashes are recorded on the bundle metadata so a replay can detect "the catalog has evolved since this bundle was built" and the operator can choose between deterministic replay (use the recorded hashes) and current-state regeneration.
- The validator's whitelist for standard W3C namespaces (`rdf:`, `rdfs:`, `xsd:`, `owl:`, `prov:`, `dcterms:`) is a fixed code-level constant; extensions to it require a code change, not an artifact authoring. This is the only departure from the "facts come from the bundle" rule, justified by the W3C set being genuinely external to dec.

### Error handling

- `Error::CatalogIncomplete { missing_fields }` — bundle assembly found zero active artifacts for one of the five fields **and** the field has no env-type default. Returned by the assembler before worker dispatch; the operator sees a clear "the catalog needs CR/OD/EX content before this dispatch can run."
- `Error::ProposalReferencesOutOfBundle { violations }` — the validator's rejection path. Includes the violation list so the operator can act.
- `Error::CapabilityVersionMismatch { dec_version, available }` — no `CapabilityReference` matches the running `dec` binary's version. Suggests the operator either upgrade dec or author a fresh reference for the running version.
- All three errors emit a `dec:Feedback` with `class = "gap"` per ADR-066 Rule 3. Routing per FT-029.

### Boundaries

- **In scope.** The five new `VerifyGraphAuthorInput` fields (Rust + Pydantic), the bundle assembler logic (one query template + one struct-builder per field), the worker prompt update, the dispatch-time validator (regex + lightweight SPARQL parser + membership checks), the `dec:concreteCapabilities` extension on `dec:VerificationEnvironment`, the env-type default table, structured `gap` feedback emission on validator rejection, integration tests against fixture catalog states (full, sparse, empty), regenerating the FT-097..FT-100 graphs as a smoke test of the closed loop.
- **Out of scope.** The catalog artifact types themselves ([FT-101](FT-101)). The slice-3 runner ([FT-098](FT-098)) — the validator described here runs at *authoring* dispatch, not at *execution* dispatch; a sibling validator on the runner side is a future feature. Auto-retry of the worker after rejection. Per-stream catalog overlays (one stream → one catalog in v1). Migration of the four placeholder VGs from the FT-097..FT-100 mint sweep into proper exemplars (a manual cleanup separately tracked).

## Out of scope

- Catalog artifact types.
- Slice-3 runner.
- Auto-retry on validator rejection.
- Per-stream catalog overlays.
- Migration of placeholder VGs.
- A web/dashboard surface for the gap-feedback rate (a fitness function consumer that reads the existing feedback artifacts; not part of this slice).
- Cross-worker bundle enrichment — the principle applies to every future graph-authoring worker, but the implementation for refactor-graph-author / deployment-graph-author / etc. is a separate slice each. This slice ships the verify-graph-author path only.
