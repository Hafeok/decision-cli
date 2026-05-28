---
id: TC-176
title: dec product verb produces byte-identical stdout to standalone product verb for representative reads
type: exit-criteria
status: failing
validates:
  features:
  - FT-105
  adrs: []
phase: 1
runner: cargo-test
runner-args: tc_176_dec_product_verb_produces_byte_identical_stdout_to
runner-timeout: 120
last-run: 2026-05-28T08:49:11.449561482+00:00
last-run-duration: 0.5s
failure-message: "warning: function `handler_internal` is never used\n  --> crates/decision-cli/src/features/loop_inspect/mod.rs:85:4\n   |\n85 | fn handler_internal(detail: String) -> HandlerError {\n   |    ^^^^^^^^^^^^^^^^\n   |\n   = note: `#[warn(dead_code)]` (part of `#[warn(unused)]`) on by default\n\nwarning: missing documentation for a variant\n  --> crates/decision-cli/src/core/dispatch_session.rs:37:5\n   |\n37 |     Completed,\n   |     ^^^^^^^^^\n   |\n   = note: requested on the command line with `-W missing-docs"
---

## Claim

For every product-cli verb in the asserted-parity set, invoking `dec product <verb> <args>` produces stdout byte-identical to invoking standalone `product <verb> <args>` against the same fixture state. Exit codes match. Stderr may differ only in the deprecation-shim warning case (covered explicitly).

## Scenarios

### Setup

- A temp directory seeded with a minimal `.product/` fixture (a few features, ADRs, TCs, and a `config.toml`) so both the new `dec product *` path and the legacy standalone `product` invocation have something to read.
- The decision-cli workspace built locally so both `dec` and the deprecation-shim `product` binaries are on `$PATH`.
- A reference `product` binary built from `crates/product-cli/` directly (i.e. the in-workspace product-cli, not the deprecation shim). This is the parity baseline — the `dec product` path must match it.

### Parity set (the verbs the test asserts)

| Verb | Args | Why included |
|---|---|---|
| `feature show` | `FT-001` | Most-used read; verifies clap arg passthrough + JSON-ish output formatting. |
| `feature list` | (none) | List rendering; verifies table-format parity. |
| `feature list` | `--phase 1 --status complete` | Filter-flag passthrough. |
| `adr show` | `ADR-001` | Sibling read path; different artifact type. |
| `adr list` | `--format json` | Format flag + JSON output parity. |
| `context` | `FT-001 --depth 2` | Bundle assembly — exercises the depth flag and recursive traversal. |
| `preflight` | `FT-001` | Computed-result verb; exercises the cross-cutting check that drove this whole conversation. |
| `graph check` | (none) | System-wide read with structured warnings. |
| `feature next` | (none) | Verb with no args; exercises the empty-args path. |

### Scenario A — byte-identical stdout

For each verb in the parity set, run both forms and assert:

```bash
dec_out=$(dec product <verb> <args>)
product_out=$(product <verb> <args>)
[[ "$dec_out" == "$product_out" ]] || { echo "MISMATCH on <verb>"; diff <(echo "$dec_out") <(echo "$product_out"); exit 1; }
```

If the comparison fails, the test prints the diff. The test fails as soon as any verb mismatches.

### Scenario B — exit-code parity

For the same set, assert that both forms exit with the same code:

```bash
dec product <verb> <args>; dec_rc=$?
product <verb> <args>; product_rc=$?
[[ $dec_rc == $product_rc ]]
```

Tested under both happy-path inputs and known-error inputs (e.g. `dec product feature show FT-NONEXISTENT` and `product feature show FT-NONEXISTENT` must both exit 1).

### Scenario C — JSON output structural equality

For verbs that take `--format json`, the test asserts **structural** equality of the JSON (not just byte equality), because field ordering in JSON dumps may legitimately differ. Use `jq -S .` to canonicalise both outputs before comparison.

### Scenario D — deprecation shim warning is the only stderr difference

The deprecation shim `product` binary emits `"warning: 'product' is deprecated; prefer 'dec product <verb>'"` on stderr in addition to whatever the verb itself writes. The test asserts:

- `dec product <verb> <args>` stderr does NOT contain the deprecation warning.
- `product <verb> <args>` (via the shim) stderr DOES contain the deprecation warning AND everything else the verb would normally write to stderr.
- After stripping the deprecation warning line, the two stderrs are byte-identical.

### Scenario E — new-only verbs are documented

For any verb that exists in `dec product *` but not in standalone `product`, OR vice versa, the test maintains a `KNOWN_DIVERGENCE` list with a comment explaining the divergence. Drift between the two surfaces (a verb added on one side but not the other) without a corresponding entry in the list is a test failure. This is the regression-detection that keeps the absorption clean.

## Runner

`bash tests/scripts/tc-176-dec-product-parity.sh`. The script:

1. Sets up the fixture `.product/`.
2. Builds the workspace with `cargo build --workspace --release`.
3. Locates both binaries (`target/release/dec`, `target/release/product`).
4. Iterates the parity set, asserting per Scenario A/B/C/D.
5. Loads the `KNOWN_DIVERGENCE` list and checks against the actual `dec product --help` and `product --help` outputs.
6. Exits 0 on full pass, 1 with the first failing verb's diff on any failure.

## Non-goals

- Asserting that every product-cli verb has identical implementation (the parity is observable stdout, not internal control flow).
- Performance comparison (out of slice; absorption is an architectural change, not a perf optimisation, though latency drops are expected as a side effect).
- MCP tool parity (TC-177 covers that).
- Behaviour against malformed fixtures (out of slice; product-cli's own tests cover that).