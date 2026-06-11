---
id: TC-423
title: Archetype chokepoint + E020 + W104 — store-side gates fire through the StreamWriter write path
type: invariant
status: passing
validates:
  features:
  - FT-147
  adrs:
  - ADR-082
  - ADR-084
  - ADR-085
phase: 1
runner: cargo-test
runner-args: -p dec-graph archetype::tests
runner-timeout: 300
observes:
- exit-code
- stdout
last-run: 2026-06-11T17:46:12.116126695+00:00
last-run-duration: 0.2s
---

## Purpose

FT-147 §Behaviour 2–3 store-side gates, exercised through the real `StreamWriter` chokepoint (ADR-041):

1. E102 fires **inside `StreamWriter::commit`** — the chokepoint registration, not just the pure validator.
2. A well-formed candidate commits, and the W104 readiness walk reports it (`W104_ArchetypePromotionReady`, informational).
3. Minting `status: standard` outside the promote path is refused with `E020_ArchetypeStatusOutsidePromotePath` (ADR-085 §6); the `StatusWriteAuthority::PromotePath` reserved for FT-158's CLI is allowed.
4. Changing a stored status (candidate → quarantined) outside the promote path is refused with E020.
5. W104 stays silent when the recorded evidence is weak.

## Mechanism

`cargo test -p dec-graph archetype::tests` — runs all five store-side tests in `crates/dec-graph/src/ontology/archetype/tests.rs` against an in-memory store with a bootstrapped stream writer.

## Pass criteria

Observed surfaces: exit-code and stdout. Exit-code 0 — all five gates behave per ADR-084/ADR-085.

## Fail criteria

Exit-code non-zero; stdout names the gate that regressed.

## Notes

W104 is the FT-147-era approximation of ADR-085 §1 — the full four-requirement walk needs SeamAudit `monolith_bar` ([FT-152](FT-152)) and Instance artifacts ([FT-156](FT-156)); this TC pins the approximation until those land. The spec placed W104 in `product graph check`; archetypes live in dec's orchestration store, so the walk ships dec-side (`promotion_ready_candidates`) for FT-158's CLI to surface.