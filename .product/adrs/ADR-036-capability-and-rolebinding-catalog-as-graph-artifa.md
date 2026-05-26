---
id: ADR-036
title: Capability and RoleBinding catalog as graph artifacts, not config files
status: accepted
features:
- FT-054
- FT-058
- FT-055
supersedes: []
superseded-by: []
domains:
- data-model
- storage
scope: domain
content-hash: sha256:9203a523074e14606d46d7f972dabbb32fc74c17439ecc8271b0544c6944d47e
amendments:
- date: 2026-05-22T18:47:09Z
  reason: Updated PRD §10.7 fixes the bootstrap path layout (`config/capabilities.yaml`, `config/role-bindings.yaml`, `scripts/bootstrap_catalog.py`) and specifies strict divergence-handling (script errors and exits on YAML/graph mismatch, never silently overwrites). Update to record the path constants and the divergence-handling discipline so the directional rule "YAML is doc, graph is authoritative" cannot be subverted by a re-run that silently rewrites graph state.
  previous-hash: sha256:2be396595684de8a354beb2e6583da475ecbbdd7d7342cd980eb83ae5ca1d7f6
---

## Context

[ADR-033](ADR-033) introduces `dec:Capability` and `dec:RoleBinding` as the artifacts that drive model selection. The mechanical question — *where does the catalog physically live?* — has three plausible answers:

1. **A YAML file on disk** (`config/capabilities.yaml`, `config/role_bindings.yaml`). Read at startup, walked by the dispatcher.
2. **Code constants** inside the `core::dispatcher` module — same as today's worker bindings, just with more entries.
3. **Graph artifacts** materialized once at bootstrap and read via SPARQL at dispatch time.

Each has a different relationship to the rest of the framework. The framework is built on three principles that lean hard one way:

- **Graph-as-state over event-sourced** ([ADR-002](ADR-002)). The orchestration store is the authoritative source for *operational state*. Capability bindings are operational state — they govern dispatch.
- **PROV-O for events and sessions** ([ADR-004](ADR-004)). Sessions cite the artifacts that influenced them. A session that ran qwen3-coder-30b on Scaleway should be able to cite the `Capability` that resolved to that model, with a version pin, exactly the same way it cites the bundle hash and worker binding.
- **The meta-loop reads the graph.** A meta-loop pattern that says "the verifier's qwen3-coder binding produces too many false-negatives; propose rebinding to qwen3.5-397b" requires that "the verifier's qwen3-coder binding" be a *queryable thing*, not a YAML line. Configuration files are opaque to SPARQL.

The forces pulling toward YAML are also real:

- YAML is easier to read in a PR diff than a TTL fragment.
- YAML can live in version control as a single source under change review.
- Bootstrapping a new operator's store from YAML is one command; bootstrapping from "go run these 12 graph writes" is twelve commands.

The resolution is to use both, with a clear directionality: **YAML is documentation and bootstrap input; the graph is the runtime source of truth.**

See the parent PRD: §5 (Capability catalog), §10.7 (bootstrap script), §14 (resolved question on YAML vs graph-native).

## Decision

Capability and RoleBinding catalogs are **graph artifacts at runtime, sourced from YAML at bootstrap**.

### Path layout (PRD §10.7)

```
decision-cli/
  config/
    capabilities.yaml      # initial Capability artifacts
    role-bindings.yaml     # initial RoleBinding artifacts
  scripts/
    bootstrap_catalog.py   # idempotent populator
```

Paths are repo-root-relative and fixed by this ADR. The dispatcher does not have a config option to read elsewhere; the catalog is in the graph or it does not exist.

### Bootstrap path

The `scripts/bootstrap_catalog.py` script reads both YAML files and writes one `dec:Capability` (or `dec:RoleBinding`) artifact per entry into the orchestration store, via `core::graph::GraphWriter` (so SHACL validation runs and version pins are recorded). The YAML's content hash is recorded on each artifact's `dec:bootstrap_source` field for audit. See [FT-058](FT-058).

After bootstrap, the YAML file is documentation. Operators may edit it; nothing reads it at dispatch time. The graph is authoritative.

### Idempotence and strict divergence handling

The script is idempotent **by refusal**, not by overwrite. For each YAML entry:

1. Compute a stable identifier from `(capability_id, version)` or `(role, version)`.
2. Query the graph: does an artifact with that identifier already exist?
3. If yes: compare YAML content to graph content. **If identical, skip silently. If different, error and exit** — the graph and YAML are out of sync; the human must decide (either update the YAML to match the graph, or bump the version field in YAML and re-run to create a new versioned artifact alongside the existing one).
4. If no: write the artifact via `GraphWriter`. Validate against SHACL.

This means bootstrap is safe to re-run after a fresh clone or after pulling new YAML. It **does not silently overwrite graph state**. The script's only authoritative job is the initial seed; after that, capability evolution happens through the meta-loop's normal policy-revision path (a new `Capability` version artifact is written, role bindings update to reference it).

The strict-divergence behavior is the load-bearing piece. A YAML that silently overwrites the graph would let stale operator edits clobber meta-loop revisions; the graph is supposed to be authoritative after bootstrap, and silent overwrite would undermine that. The error-and-exit path forces the human to confront the divergence and decide.

### Update path

Catalog updates flow through the same authoring path as any other artifact:

- A meta-loop session (or a human operator) writes a new `Capability` / `RoleBinding` artifact via `GraphWriter` with an incremented `dec:version` and a `dec:supersedes` link to the prior version.
- The dispatcher's resolution query (`SELECT … WHERE { ?b dec:role_id ?r ; dec:active true }`) sees the new active artifact; the prior version remains in the graph as audit history.
- Optionally, `dec capability sync` regenerates the YAML from the graph for reviewability — but the graph is the source of truth even when YAML is stale.

### Why this directionality

The YAML-only path forces a class of changes — *any* capability tweak — to be a code repo change with a PR, deployment cycle, and operator intervention. That is correct for the *catalog schema* (an ontology change is a foundational ADR — see [ADR-035](ADR-035)). It is wrong for *catalog entries*, which are exactly the things the meta-loop needs to revise.

The graph-only path (no YAML) sacrifices reviewability. Every new operator runs `dec init` and gets *some* catalog; what they get is whatever the binary's hardcoded `INITIAL_CAPABILITIES` constant said. Reviewing that constant requires reading Rust. A YAML file is a better artifact for "what does the slice-3 default catalog contain?"

Combining them with a clear directionality solves both: the YAML is what humans read; the graph is what the dispatcher reads; bootstrap copies one into the other.

### Validation at bootstrap

The bootstrap reads the YAML, constructs `dec:Capability` / `dec:RoleBinding` Turtle, and writes through `GraphWriter`. SHACL validation runs on the write. A malformed YAML entry (missing required field, bad enum value) fails bootstrap with a specific error pointing at the YAML line; the partial bootstrap is rolled back (graph-write is atomic per [FT-001](FT-001)).

This means SHACL on `dec:Capability` ([FT-054](FT-054)) and `dec:RoleBinding` ([FT-055](FT-055)) is the *only* schema validation; the YAML loader does not duplicate schema checks. The YAML loader is a serialization adapter, not a validation layer.

### Ordering constraint

The script enforces strict ordering: **capabilities are bootstrapped first, then role-bindings**. A role-binding YAML entry that references a capability not yet present in the graph would fail SHACL with a confusing "unknown reference" error; ordering avoids this. The script will reject role-bindings.yaml entries whose `default_capability` is not in capabilities.yaml of the same run.

## Consequences

**Positive.**

- The meta-loop can revise capability bindings as a graph mutation, same as it revises any other artifact. There is no "but capability changes are special" carve-out.
- New operators get a reviewable starting catalog (the YAML in the repo). The PR review burden lands on a YAML file, not a Rust constant.
- The graph is the single audit source. Session records cite Capability artifacts by version; the meta-loop reads the same artifacts; the dispatcher resolves through them. Three paths, one truth.
- Adding new endpoints (a fictional `gcp-vertex` endpoint) requires only a new YAML entry and (separately) a client wrapper. No dispatcher changes, no ontology changes.
- Strict divergence handling means an operator cannot silently lose meta-loop revisions by re-running bootstrap with stale YAML. The error path forces a deliberate decision.

**Negative / accepted costs.**

- Drift between YAML and graph is *expected* once the meta-loop starts evolving capabilities. An operator who hand-edits the YAML expecting it to take effect at next dispatch will be confused. The mitigation (`dec capability sync` to regenerate YAML from graph) is real but is one more command operators must learn.
- Bootstrap is now a load-bearing step. `dec init` failing means no catalog, which means no dispatch. The graph-write atomicity from [FT-001](FT-001) handles partial failures; SHACL surfaces schema errors clearly.
- Two artifacts must agree (YAML in repo + graph at runtime). The mitigation is to treat the YAML as documentation only — operators reading the YAML to understand the catalog is fine; operators editing the YAML expecting runtime changes is not.
- A re-bootstrap after meta-loop revisions will error out (mismatch). This is intended — but it means deploying onto an existing store from a fresh clone requires the operator to either skip bootstrap (graph already has the catalog) or accept that the YAML is documentation only.

**Boundary enforcement.**

- The YAML file paths are fixed (`config/capabilities.yaml`, `config/role-bindings.yaml`). The dispatcher does not have a config option to read elsewhere; the catalog is in the graph or it does not exist.
- `dec init --skip-catalog` is *not* offered. Operators get the seeded catalog or they fail init explicitly.
- Worker binaries do not read the YAML or the graph for capability data. They read the dispatch payload. [ADR-008](ADR-008) is preserved.
- The bootstrap script must never silently overwrite graph state. Mismatch errors out.

## Relationship to existing ADRs

- **[ADR-002](ADR-002) (graph-as-state).** Direct application — capability catalog is operational state, lives in the graph.
- **[ADR-006](ADR-006) (definition documents over raw strings for value stream init).** Pattern parallel — definition documents for value streams, YAML seed for capabilities; both are "source documents that init reads once and the graph then owns".
- **[ADR-007](ADR-007) (embedded base ontology).** Different layer — the ontology is embedded as binary bytes; capability *instances* are seeded via YAML. The new `dec:Capability` class definition is part of the embedded ontology; the catalog entries are not.

## Status

Proposed. Governs [FT-058](FT-058) (catalog bootstrap). Companion to [ADR-033](ADR-033) (capability routing). Path layout and strict divergence-handling recorded by amendment per PRD §10.7.
