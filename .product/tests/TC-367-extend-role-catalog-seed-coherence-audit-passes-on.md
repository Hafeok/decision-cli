---
id: TC-367
title: extend-role-catalog-seed coherence audit passes on positive fixture with consistent IRI references
type: scenario
status: passing
validates:
  features:
  - FT-144
  adrs: []
phase: 1
runner: bash
runner-args: scripts/checks/tc-367-cluster-audit-role-catalog-seed-positive.sh
runner-timeout: 60
observes:
- exit-code
- stderr
last-run: 2026-06-04T15:47:46.704677747+00:00
last-run-duration: 0.0s
---

## Context

Positive coherence-audit TC for [FT-144](FT-144) — asserts the `extend-role-catalog-seed` audit (`scripts/checks/cluster-audit-extend-role-catalog-seed.py`) passes on a synthetic positive fixture where all six cells emit consistent IRI references, the wiring line exists, the SHACL shape and seed quads agree, the `Role` struct field and seed quad value-types agree, and `round_trip_tests` contains the required `legacy_store_lookup_returns_safe_default` test.

The audit's passing-case behaviour is part of the safety contract per [ADR-080](ADR-080): a healthy cluster MUST audit-pass, otherwise the load-bearing audit is a false-positive blocker for every cluster run.

## Setup

- A fixture directory under `tests/fixtures/cluster-audit-extend-role-catalog-seed/positive/` containing six emitted cell outputs that consume the maximal-case parameters (`requires_shacl=true, surfaces_on_role_struct=true`):
  - `iri_constants.rs` declares e.g. `pub const FOO_IRI: &str = "https://decision-cli.dev/ns/foo";` and `pub const BAR_IRI: &str = "https://decision-cli.dev/ns/bar";`.
  - `seed_quad_function.rs` declares `pub fn foo_seed_quads() -> Vec<Quad> { ... }` and references both `FOO_IRI` and `BAR_IRI` at least once each.
  - `init_pipeline_wiring.rs` contains a snippet ending in `quads.extend(foo_seed_quads());` inside the `seed_role_catalog` body.
  - `shacl_shape_extension.ttl` declares a `sh:property [ sh:path <https://decision-cli.dev/ns/foo> ; sh:datatype xsd:string ]` clause for the single new predicate that requires cardinality enforcement.
  - `role_struct_field_extension.rs` declares `pub foo: Vec<String>,` on `pub struct Role` AND a `collect_foo()` helper AND a `lookup()` snippet calling `collect_foo(...)`. Field type is `Vec<String>`, matching the seed_quad_function's string-literal object emissions.
  - `round_trip_tests.rs` contains four tests including one whose function name matches `legacy_store_lookup_returns_safe_default`.
- The wrapper script `scripts/checks/tc-367-cluster-audit-role-catalog-seed-positive.sh` invokes the audit with all six paths and `--params '{"requires_shacl": true, "surfaces_on_role_struct": true}'`.

## Steps

1. Run `scripts/checks/cluster-audit-extend-role-catalog-seed.py` against the positive fixture via the wrapper.
2. Capture exit code and stderr.

## Expected outcome

- Exit code 0 (audit pass).
- Stderr is empty OR contains only INFO-level lines listing the six PASS checks (no FAIL lines).

## Pass / fail

- Pass: bash runner exits 0 because the wrapper asserts the audit exited 0 AND no FAIL line appeared in stderr.
- Fail: the audit script exits non-zero on what should be a clean fixture, OR a FAIL line appears (false positive).

## Why this TC

Audits are useless if they false-positive — every cluster run would block. This is the baseline that proves the audit's six checks distinguish a healthy cluster from a broken one. Paired with TC-368 (negative — missing IRI) and TC-369 (negative — missing fail-closed test), the trio forms the audit's contract surface.