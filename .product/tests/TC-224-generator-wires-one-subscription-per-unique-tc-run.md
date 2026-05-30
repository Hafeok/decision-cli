---
id: TC-224
title: Generator wires one subscription per unique TC runner type observed in .product/tests
type: scenario
status: passing
validates:
  features:
  - FT-114
  adrs: []
observes:
- file
phase: 4
runner: cargo-test
runner-args: tc_224_generator_wires_one_subscription_per_runner_type
runner-timeout: 30
last-run: 2026-05-30T16:41:05.759016390+00:00
last-run-duration: 0.6s
---

## Description

The generator's job is to make `dec drive ship` dispatchable
for the project's existing TCs without operator config. Each
unique `runner:` value in `.product/tests/` must produce a
subscription in the generated value-stream so the verifier
knows how to invoke that runner type.

## Acceptance Criteria

Cargo test:

1. Compose a temp `.product/tests/` with seven TCs whose
   `runner:` frontmatter fields are: `cargo-test` (×3),
   `bash` (×2), `pytest` (×1), and one TC with no `runner:`
   field at all (legacy/unimplemented).
2. Call `generate_value_stream(&product_root, "test-repo")`.
3. Parse the returned `.ttl` and count the subscription
   entries. Assert:
   - Exactly 3 subscriptions are wired: one for `cargo-test`,
     one for `bash`, one for `pytest`. (Duplicates collapse.)
   - The TC with no runner is silently skipped — no warning,
     no scaffolded subscription. (Unimplemented TCs are
     normal; ignore them.)
4. **Unknown runner case.** Add a TC with `runner: deno-test`
   (a runner the generator doesn't know about). Re-run the
   generator. Assert:
   - The subscription block contains a stubbed entry with
     `runner: deno-test` and a comment line
     `# TODO: wire — unknown runner type`.
   - The generator does NOT exit non-zero; unknown runners are
     a warning, not a hard error.