---
id: TC-169
title: Dispatch-time validator rejects a proposal that references commands, namespaces, or hosts not in the bundle
type: scenario
status: failing
validates:
  features:
  - FT-102
  adrs: []
phase: 1
runner: cargo-test
runner-args: tc_169_dispatch_time_validator_rejects_a_proposal_that_re
runner-timeout: 120
last-run: 2026-05-28T08:49:05.201345116+00:00
last-run-duration: 0.9s
failure-message: "warning: function `handler_internal` is never used\n  --> crates/decision-cli/src/features/loop_inspect/mod.rs:85:4\n   |\n85 | fn handler_internal(detail: String) -> HandlerError {\n   |    ^^^^^^^^^^^^^^^^\n   |\n   = note: `#[warn(dead_code)]` (part of `#[warn(unused)]`) on by default\n\nwarning: missing documentation for a variant\n  --> crates/decision-cli/src/core/dispatch_session.rs:37:5\n   |\n37 |     Completed,\n   |     ^^^^^^^^^\n   |\n   = note: requested on the command line with `-W missing-docs"
---

## Claim

The dispatch-time completeness validator walks every proposed step's referenced facts (binaries, `dec` subcommands, SPARQL namespaces, HTTP hosts, writable paths, capture sources) and refuses persistence when any referenced fact is not in the bundle that was sent to the worker. Standard W3C namespaces (`rdf:`, `rdfs:`, `xsd:`, `owl:`, `prov:`, `dcterms:`) are whitelisted.

## Scenarios

### Setup

- Temp `.dec/` with catalog seeded per TC-168's setup (CR-001, CR-002, CR-003 covering specific commands; OD-001 declaring the dec namespace; EX-001, EX-002 exemplars; ENV-001 with explicit `concreteCapabilities`).
- A test-only injection seam in the `dec verify graph generate` handler that lets the test substitute a fixed `GraphProposal` for the worker's output. (Without this the test depends on LLM behaviour; with it, the test asserts on the validator independently.)

### Scenario A — happy path: every reference is in the bundle

Inject a `GraphProposal::New` whose steps reference only:

- binary `dec`, subcommand `dec verify graph new` (in CR-001).
- SPARQL namespace `https://decision-cli.dev/ns#` (in OD-001) plus `rdf:`, `xsd:` (whitelisted).
- writable path `$DEC_VERIFY_TMP/result.ttl` (in env_capabilities.writable_paths).
- capture source `prior_step_stdout` referencing step 0.

Run the generate verb to completion. Assertions:

- Exit code: 0.
- `VG-NNN.ttl` is persisted under `.dec/verify/graph/`.
- No `dec:Feedback` artifact of class `gap` is emitted.

### Scenario B — unknown binary is rejected

Inject a proposal whose first step is `shell-command` with `command = "curl https://example.com"`. `curl` is not in `env_capabilities.binaries_on_path`. Assertions:

- Exit code: 1.
- Stderr contains `ProposalReferencesOutOfBundle`.
- The violation list (printed on stderr or in the structured error) names: `{ step_index: 0, kind: "binary", referenced_thing: "curl", why_rejected: "not in env_capabilities.binaries_on_path" }`.
- No `VG-NNN.ttl` is persisted.

### Scenario C — unknown `dec` subcommand is rejected

Inject a proposal with `command = "dec verify result inspect VGR-001"`. `dec verify result inspect` is not in `cli_surface.dec_subcommands`. Assertions:

- Violation: `{ kind: "dec_subcommand", referenced_thing: "dec verify result inspect", why_rejected: "not in cli_surface.dec_subcommands" }`.
- Exit code: 1; no persistence.

### Scenario D — unknown SPARQL namespace is rejected

Inject a proposal with `sparql-assertion` step `query = "PREFIX foo: <https://fake.example/ns#> SELECT * WHERE { ?s foo:p ?o }"`. The namespace is neither in `ontology_vocabulary.namespaces` nor in the W3C whitelist. Assertions:

- Violation: `{ kind: "sparql_namespace", referenced_thing: "https://fake.example/ns#", why_rejected: "not in ontology_vocabulary.namespaces and not in W3C whitelist" }`.
- Exit code: 1; no persistence.

### Scenario E — W3C-whitelisted namespace is allowed

Inject a proposal with `sparql-assertion` step using `PREFIX prov: <http://www.w3.org/ns/prov#>` and a query against `prov:Activity`. Assertions:

- Exit code: 0. No violation. Persisted normally.

### Scenario F — file path outside writable_paths is rejected

Inject a `file-assertion` step with `target = "/etc/passwd"`. `/etc/...` is not in `env_capabilities.writable_paths`. Assertions:

- Violation: `{ kind: "file_path", referenced_thing: "/etc/passwd", why_rejected: "not in env_capabilities.writable_paths" }`.
- Exit code: 1; no persistence.

### Scenario G — HTTP host outside allowed_hosts is rejected

Switch env to a `remote-http` variant with `allowed_hosts = ["api.dec.test"]`. Inject an `http-request` step with `url = "https://evil.example/probe"`. Assertions:

- Violation: `{ kind: "http_host", referenced_thing: "evil.example", why_rejected: "not in env_capabilities.allowed_hosts" }`.
- Exit code: 1; no persistence.

### Scenario H — multiple violations are all reported

Inject a proposal with 3 steps each introducing one violation. Assertions: the violation list has length 3 and reports each step independently — the validator does not short-circuit on the first failure.

## Runner

`bash tests/scripts/tc-169-validator-rejection.sh`. The injection seam is a flag/env-var on `dec verify graph generate` (e.g. `DEC_VERIFY_GRAPH_GENERATE_INJECT_PROPOSAL=<json-path>`) that bypasses the worker subprocess and feeds the recorded proposal directly to the validator+persistence path. The test ships fixture proposals (one per scenario) as JSON files.

## Non-goals

- LLM compliance with the prompt instructions (out of slice; the prompt aims to keep proposals in-bundle, the validator catches when it doesn't).
- The chain-integrity gate's reaction to a rejected proposal (FT-047; out of slice — the chain-integrity gate consumes coverage, which is unaffected by validator rejection).
- Auto-retry behaviour (explicitly out of scope per ADR-066 Rule 3).