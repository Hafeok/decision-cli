---
id: TC-270
title: Dispatch JSON written to worker stdin contains non-empty allowed_tools when role is seeded
type: scenario
status: unimplemented
validates:
  features:
  - FT-122
  adrs:
  - ADR-008
phase: 4
observes:
- stdout
runner: cargo-test
runner-args: tc_270_dispatch_json_has_allowed_tools
runner-timeout: 30
---

## Description

The wire-shape assertion paired with TC-269. TC-269 covers the in-memory struct; this TC covers the JSON that actually crosses the harness/worker boundary. The two together close the wire-format contract for FT-122.

## Acceptance Criteria

Reuse the existing test pattern in `crates/decision-cli/src/features/implement/worker.rs::tests` (around lines 280-313) that captures the payload via `install_mock`. The mock receives a `&DispatchPayloadJson`; serialise it via `serde_json::to_string(&payload)` inside the test and assert against the string:

- The serialised JSON contains the substring `"allowed_tools":[`.
- Parsing the JSON via `serde_json::from_str::<serde_json::Value>(...)` yields `Value::Object(..)` with key `"allowed_tools"` mapping to a JSON `Array` of five string elements.
- The five elements (in any order) are: `"read_file"`, `"write_file"`, `"run_build"`, `"run_lint"`, `"run_tests"`.

Lives at `crates/decision-cli/src/features/implement/worker.rs::tests::tc_270_dispatch_json_has_allowed_tools`. Uses the existing `install_mock` helper to intercept the payload without spawning a real worker subprocess.
