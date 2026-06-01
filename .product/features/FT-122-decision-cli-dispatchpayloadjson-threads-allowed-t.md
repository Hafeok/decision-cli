---
id: FT-122
title: 'decision-cli: DispatchPayloadJson threads allowed_tools from role catalog to worker stdin'
phase: 4
status: planned
depends-on:
- FT-121
- FT-066
adrs:
- ADR-008
- ADR-070
- ADR-071
tests:
- TC-269
- TC-270
- TC-271
domains:
- data-model
- api
domains-acknowledged:
  data-model: "One additive `serde(default)` field on `DispatchPayloadJson`. Backwards-compatible by construction — old payloads parse as empty Vec; new payloads parse against either generation of worker."
  api: "Extends the worker dispatch payload (the harness↔worker contract) with one new field. Wire-format change, monotonic and backwards-compatible — old payloads parse as empty Vec, new payloads parse against either generation of worker."
---

## Description

FT-121 puts `allowed_tools` on the `Role` struct. This feature threads it onto the dispatch payload so the worker actually receives it. After it lands, the JSON the Rust harness writes to the code-writer worker's stdin contains an `"allowed_tools": [...]` array sourced from the role catalog lookup — and the Python worker has the data it needs (in FT-123) to enforce the surface.

The change is small and additive: one new field on `DispatchPayloadJson` (Rust), one population step in `build_dispatch_payload()`, one field-read on the Python side (the Pydantic field already exists at `models.py:87`; nothing to add there).

Backwards-compatibility is by construction. The new Rust field is `#[serde(default)]`, so old workers receiving a new payload see an empty Vec (matching today's Python default). New workers receiving an old payload (from a harness that pre-dates this FT) also see an empty Vec → fail-closed per [ADR-069](ADR-069). The wire-format change is monotonic.

## Functional Specification

### Inputs

No operator-facing inputs. The harness reads `allowed_tools` from `role_catalog::lookup(&ctx.store, IMPLEMENTER_ROLE)` during dispatch assembly; the field is internal to the harness/worker boundary.

### Outputs

- `crates/decision-cli/src/features/implement/worker.rs::DispatchPayloadJson` gains:
  ```rust
  #[serde(default)]
  pub allowed_tools: Vec<String>,
  ```
- `crates/decision-cli/src/features/implement/lifecycle.rs::build_dispatch_payload()` populates the field from the catalog lookup.
- The JSON written to the worker's stdin contains `"allowed_tools": ["read_file", "write_file", ...]` when the catalog is seeded (post-FT-121), or `"allowed_tools": []` against legacy stores.

### Behaviour

1. Extend `DispatchPayloadJson` with `allowed_tools: Vec<String>` (additive field, serde default empty).
2. In `build_dispatch_payload()` (`lifecycle.rs:25-52`), call `role_catalog::lookup(&ctx.store, IMPLEMENTER_ROLE_IRI)`. Use the returned `Role.allowed_tools`. If lookup returns `Ok(None)` (no role in catalog) or `Err(_)`, populate `allowed_tools: vec![]` and continue — the worker fail-closes per [ADR-069](ADR-069). Log the lookup failure at `warn!` level (visibility, not a hard stop, since the worker will fail loudly downstream).
3. JSON write path is unchanged. The existing serde derive handles the new field automatically; no manual JSON construction code touches it.
4. Python worker reads `payload.allowed_tools` via the existing Pydantic field; FT-123 wires it into the loop's tool-list filter. This feature only delivers the data — it does not change worker behaviour.

### Acceptance criteria

- A freshly seeded store (post-FT-121) dispatching the implementer role produces a `DispatchPayloadJson` with `allowed_tools == vec!["read_file", "write_file", "run_build", "run_lint", "run_tests"]`.
- The JSON serialisation of that payload contains `"allowed_tools":["read_file","write_file","run_build","run_lint","run_tests"]` (order-insensitive comparison, but the field must be present and non-empty).
- A legacy store (pre-FT-121) dispatching the implementer role produces a `DispatchPayloadJson` with `allowed_tools == vec![]`. The harness emits a `warn!` log entry naming the missing seed. The dispatch JSON contains `"allowed_tools":[]`.
- A worker run against an empty-`allowed_tools` payload exits with a structured `WorkerResponse(status="error", error.category="invalid_dispatch", error.message=~"no tools granted")`. This is asserted via the existing pytest harness against the Python `models.DispatchPayload`.

### Non-goals

- Worker-side enforcement of the surface (i.e. the agentic loop's tool registry filter). Owned by FT-123.
- Surface declaration for non-implementer roles. The harness lookup uses the implementer role IRI; other roles (verifier, future reviewer) thread their own `allowed_tools` via the same path when the dispatcher integrates them — out of scope here.
- A migration script that retroactively seeds `dec:roleTool` quads into legacy stores. Operators re-seed via `dec init --from` per FT-121's non-goals.

## Exit Criteria (Test Coverage)

Per [ADR-013](ADR-013), behaviours above are asserted by TCs linked to this feature.
