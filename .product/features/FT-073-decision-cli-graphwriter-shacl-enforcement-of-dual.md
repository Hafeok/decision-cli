---
id: FT-073
title: 'decision-cli: GraphWriter SHACL enforcement of dual provenance at write time'
phase: 3
status: complete
depends-on:
- FT-072
adrs:
- ADR-041
- ADR-013
- ADR-016
tests:
- TC-123
domains:
- data-model
- error-handling
domains-acknowledged:
  ADR-037: ADR-037 governs Scaleway/Anthropic provider defaults. This feature does not configure provider routing.
  ADR-014: ADR-014 governs Architectural Fitness Functions as product-cli artifacts. This feature does not introduce a new fitness function.
  ADR-044: ADR-044 governs Brief as a typed artifact in product-cli's catalog. This feature was not authored from a Brief.
  ADR-033: ADR-033 governs capability-based model routing as a graph-resident layer. This feature does not route models.
  ADR-018: ADR-018 governs the VerificationVerdict schema. This feature does not produce a verification verdict.
  ADR-004: ADR-004 governs PROV-O event and session shapes. This feature does not introduce new event or session types.
  ADR-035: ADR-035 governs Bundle.stakes as a first-class judgment field. This feature does not assemble a stakes-bearing bundle.
  ADR-039: ADR-039 governs motivational predicates as rdfs:subPropertyOf prov:wasDerivedFrom. This feature does not introduce new motivational predicates.
  ADR-002: ADR-002 governs graph-as-state vs event-sourced semantics. This feature's scope does not change that choice.
  ADR-034: ADR-034 governs tiered escalation policy with controlled trigger vocabulary. This feature does not invoke escalation.
  ADR-040: ADR-040 governs the BoundaryArtifact class. This feature does not introduce a new boundary artifact.
  ADR-023: ADR-023 governs the Feedback controlled vocabulary. Not invoked here.
  ADR-055: ADR-055 governs WorkerImage as a catalog mirroring the Model catalog. This feature does not extend that catalog.
  ADR-001: ADR-001 governs the oxi-events crate's SDP boundary. This feature does not modify oxi-events' public surface.
  ADR-021: ADR-021 governs action-interpretation agreement as a fitness metric. Not applicable without a paired action-interpretation session.
  ADR-038: ADR-038 governs dual-provenance discipline (mechanical + motivational). This feature does not introduce a new artifact type subject to dual provenance.
  ADR-017: ADR-017 governs action-interpretation pairing as a structural requirement. This feature does not produce an action-interpretation pair.
  ADR-064: ADR-064 governs LiteLLM as the LLM-call substrate. This feature does not call LiteLLM.
  ADR-012: ADR-012 governs per-stream working-directory discovery. This feature does not introduce a stream-bound command.
  data-model: Domain 'data-model' is in scope of this feature; not paving in extra cross-cutting governance beyond the linked ADRs.
  ADR-047: ADR-047 governs capability-tag binding via catalog at dispatch time. This feature does not perform capability-tag-to-entry binding.
  ADR-036: ADR-036 governs the Capability and RoleBinding catalog as graph artifacts. This feature does not extend that catalog.
  ADR-054: ADR-054 governs LiteLLM as the worker SDK's provider substrate. This feature does not call LiteLLM.
  ADR-025: ADR-025 governs blocking vs non-blocking Feedback semantics. Not invoked here.
  ADR-043: ADR-043 governs full-chain traversal as a QueryTemplate artifact. This feature does not introduce a new full-chain query.
  ADR-024: ADR-024 governs the Feedback lifecycle state machine. Not invoked here.
  ADR-065: ADR-065 governs the Dagger deferral for the worker runtime model. This feature does not depend on the runtime model.
  ADR-005: ADR-005 governs value-stream-resident scope. This feature is not value-stream-scoped.
  ADR-022: ADR-022 governs Feedback as a first-class flow class. This feature does not produce Feedback artifacts.
  ADR-027: ADR-027 governs authority declarations in the role catalog. This feature does not register a new role.
---

## Description

Add a SHACL validation stage to GraphWriter (FT-001's mutation chokepoint) that enforces the dual-provenance discipline on every write. Validation runs against the shape file set (FT-072) composed with the live named-graph snapshot before commit. Non-conformant writes are rejected; violations are emitted as structured `ProvenanceViolation` Feedback artifacts routed back to the producing session.

This is the feature that turns the discipline from "shape files exist" into "the graph maintains the invariant continuously" (ADR-041).

## Functional Specification

### Inputs

- The shape file set loaded at bootstrap (FT-072).
- The incoming triple delta from a GraphWriter transaction (prepared, not yet committed).
- A read-snapshot of the current named graph for cross-artifact constraint evaluation.
- The session record handed in by the harness's session-completion handler (used by GraphWriter to *materialise* the mechanical block before validation runs).

### Outputs

- `crates/decision-cli/src/core/graph/shacl.rs` (or equivalent) — a `Validator` struct wrapping oxigraph-shacl with the loaded shape set.
- `core/graph/writer.rs` extended with a `validate_and_commit(triples)` path that:
  1. Materialises mechanical-provenance triples from the session record onto every artifact in the delta.
  2. Runs SHACL validation against `(materialised_delta ∪ current_snapshot)`.
  3. Commits on conformance; emits `ProvenanceViolation` and returns `WriteError::ProvenanceRejected` on non-conformance.
- `core/graph/violation.rs` — the `ProvenanceViolation` struct and its serialisation to the standard Feedback artifact shape (ADR-022).
- Python SDK side: `workers/_shared/shacl.rs` (or `.py`) running pyshacl with the same shape set as a defensive pre-check before workers hand artifacts back.
- A new TC asserting that a write missing mechanical provenance from a forced bypass path fails with a structured violation.

### State

- The `Validator` is constructed once at `dec init` from the loaded shape set and reused for the lifetime of the process. Shape files are immutable per ADR-041's load model.
- No new persistent state — violations are emitted as Feedback artifacts, which already have their own persistence story (FT-026).

### Behaviour

1. **Pre-commit pipeline.**
   ```rust
   pub fn validate_and_commit(
       &self,
       delta: TripleDelta,
       session: &SessionRecord,
   ) -> Result<CommitReceipt, WriteError> {
       let delta = self.materialise_mechanical_block(delta, session)?;
       let snapshot = self.store.read_snapshot()?;
       let report = self.validator.validate(&delta, &snapshot)?;
       if !report.conforms {
           let violation = ProvenanceViolation::from_shacl_report(&report);
           self.emit_violation_feedback(&violation, session)?;
           return Err(WriteError::ProvenanceRejected(violation));
       }
       self.store.commit(delta)
   }
   ```

2. **Mechanical materialisation.** For every artifact IRI in the delta that does not yet carry mechanical triples, GraphWriter asserts:
   - `?artifact prov:wasGeneratedBy <session_iri>`
   - `?artifact prov:wasAttributedTo <agent_iri>` for each Agent associated with the session
   - `?artifact prov:generatedAtTime <transaction_timestamp>`
   The timestamp is the GraphWriter transaction clock at the moment of validation, single-writer per graph per ADR-002.

3. **Validation failure modes.**
   - **Missing mechanical provenance after materialisation.** Internal bug — the session record was malformed or the materialisation pass missed an artifact. `panic!` with full context; never reachable in correctly-wired flow.
   - **Missing motivational provenance AND not a BoundaryArtifact.** `WriteError::ProvenanceRejected`. `ProvenanceViolation` payload names the artifact, its declared type, the set of motivational predicates the type accepts (looked up from the shape), and the failed assertion.
   - **Type-specific shape violation** (required field missing, edge with wrong-typed target, `:external_origin` missing on a BoundaryArtifact). Same rejection pattern; report names the failed `sh:property` paths.

4. **Violation routing.** Every rejection produces a Feedback artifact of class `provenance-violation` (extends FT-028's vocabulary if needed; otherwise reuses the existing structural-violation class) routed to the producing session via the standard FT-029 routing table. For boundary-rejection cases (a CI submission), routing falls through to the operator-curator role.

5. **Subclass-aware `sh:class` reasoning.** The validator is configured with `rdfs:subClassOf` reasoning enabled (resolves Brief open question 2). `sh:class dec:Feedback` accepts instances of any subclass declared `rdfs:subClassOf dec:Feedback`.

6. **Cross-side defensive validation.** The Python SDK runs pyshacl on the worker side before handing artifacts back via the ADR-008 contract. Same shape files as the Rust side. Workers surface violations as worker-side errors with the same `ProvenanceViolation` shape; the harness re-validates and is authoritative.

7. **Build-time agreement check.** A CI fitness function constructs a fixture artifact deliberately violating one constraint at a time and asserts both validators (Rust and Python) reject identically. Drift is a build break.

### Invariants

- **GraphWriter is the only path that writes mechanical triples.** No worker, no feature code, no test fixture authors `prov:wasGeneratedBy` directly. The chokepoint owns this.
- **Validation is synchronous with commit.** A write either passes validation and commits, or fails validation and rejects. There is no "commit and validate later" path.
- **The snapshot used for cross-artifact constraints is the snapshot the commit would have produced.** GraphWriter holds the read-snapshot for the duration of the validate-and-commit transaction; reads are serializable with the write.
- **Validation latency budget: < 50 ms p99 on slice-1 reference workload.** Measured by a fitness TC; regression escalates to operator review.
- **Same shape files on both sides.** Build-time fitness check (FT-072) enforces byte-identity; this feature's runtime fitness check enforces decision-identity (same artifact → same conformance verdict on both validators).

### Error handling

- `WriteError::ProvenanceRejected(violation)` returned to the caller (harness's session-completion handler). Caller decides whether to fail the session or surface the violation as recoverable feedback.
- `WriteError::ShapeLoadFailure` returned at orchestration-store init if shape files cannot be loaded. Fatal at `dec init`.
- `panic!` if mechanical materialisation fails (session record malformed) — internal bug.
- `WriteError::Internal` wrapping oxigraph-shacl errors that are not shape-load and not violation; treated as orchestration-store corruption.

### Boundaries

- **In scope.** The `Validator` struct. The `validate_and_commit` path on GraphWriter. Mechanical materialisation from session records. `ProvenanceViolation` and its Feedback emission. Subclass-reasoning configuration. The pyshacl defensive layer on the worker side. The two runtime fitness checks (latency, dual-validator agreement).
- **Out of scope.** Shape files themselves (FT-072). Migration tooling (FT-074). Continuous chokepoint-bypass fitness function (slice 2+ per Brief excludes).

## Out of scope

- Backwards-compatible "warn but allow" mode on a per-instance basis. Slice-1 uses warn-only mode *globally* during the FT-074 migration window; after cutover, validation rejects on every write.
- Validation of read paths. SHACL is a write-time concern; reads do not re-validate.
- Performance optimisation of oxigraph-shacl itself. Slice 1 ships working at the < 50 ms budget; deeper tuning is later work.

## References

- [ADR-041](ADR-041) — SHACL as enforcement mechanism (the decision this feature implements).
- [ADR-008](ADR-008) — Worker contract (preserved: workers do not query the graph; defensive validation runs SDK-side before handing artifacts back).
- [FT-001](FT-001) — GraphWriter chokepoint (the actor this feature extends).
- [FT-072](FT-072) — Shape files this validator consumes.
- [FT-026](FT-026), [FT-028](FT-028), [FT-029](FT-029) — Feedback artifact, vocabulary, routing.
