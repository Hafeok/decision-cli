---
id: TC-088
title: verifier role-catalog entry resolves to a known role at dispatch
type: exit-criteria
status: failing
validates:
  features:
  - FT-019
  adrs:
  - ADR-027
phase: 2
runner: cargo-test
runner-args: tc_088_verifier_role_catalog_resolves
runner-timeout: 120
last-run: 2026-05-21T13:31:41.447481757+00:00
last-run-duration: 0.3s
failure-message: "   Compiling decision-cli v0.1.0 (/home/hafeok/projects/decision-cli/crates/decision-cli)\nerror[E0432]: unresolved import `decision_cli::core::ontology::verification_env`\n  --> crates/decision-cli/tests/tc_055_dec_init_seeds_ephemeral_cli_env_idempotently.rs:16:35\n   |\n16 | use decision_cli::core::ontology::verification_env::{\n   |                                   ^^^^^^^^^^^^^^^^ could not find `verification_env` in `ontology`\n\nerror[E0432]: unresolved import `decision_cli::core::ontology::ver"
---

## Purpose

Exit criterion for [FT-019](FT-019): the verifier role-catalog entry is registered and resolvable at dispatch time. A `dec dispatch role verifier <artifact>` call (or its slice-2 equivalent) finds the role record without falling back to a default.

## Given

A workspace where `dec init` has run and the role-catalog projection is loaded into the store.

## When

```bash
dec role list | grep verifier
dec dispatch role verifier <some-artifact>   # slice-2 dispatch surface
```

## Then

- `dec role list` includes a row with role id `verifier` whose attributes match the FT-019 catalog declaration.
- The dispatch call resolves the role successfully (does not error `RoleNotFound`).
- The session opened by the dispatch records the verifier role's authority declarations ([ADR-027](ADR-027)) in its PROV-O attributes.

## Notes

Pairs with the invariant TC-027 (every action-session has an interpretation-session) — TC-027 asserts the *pairing structure*; TC-088 asserts the *role resolution* that makes the pairing possible.