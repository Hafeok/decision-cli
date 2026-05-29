---
id: FT-118
title: 'decision-cli: bootstrap_catalog deactivates prior role-binding versions when adding new ones'
phase: 4
status: planned
depends-on: []
adrs: []
tests:
- TC-249
- TC-250
- TC-251
- TC-252
domains:
- data-model
domains-acknowledged:
  data-model: Extends the bootstrap-catalog transaction with prior-version deactivation. The data-model-domain ADRs (ADR-036 catalog discipline) govern the write atomicity; this feature enforces the existing uniqueness invariant at write time rather than read time.
---

## Description

The capability resolver enforces a uniqueness invariant:
exactly one `dec:RoleBinding` per `dec:roleId` may have
`dec:active=true` in the store at any time. Multiple active
bindings for the same role are an unresolvable contradiction
("which capability does this role default to?"), and the
resolver bails with:

```
malformed role binding <dec:role_id="X">:
2 active role bindings share the same role_id
(uniqueness invariant violated)
```

`dec init` seeds a baseline role binding (e.g.
`verify-graph-author/v1` → `code-writer/v1`) so a fresh
workdir resolves immediately. `bootstrap_catalog.py`
subsequently lays down the YAML's bindings (e.g.
`verify-graph-author/v7` → `verify-graph-author/v7`) for
operators who want non-default routing.

The bug: bootstrap is **additive** — it writes the new
binding marked `active=true` but does NOT deactivate the
init-seeded baseline. Now two active bindings for the same
role coexist in the store; the next resolver lookup fails.

Witnessed today: after switching the verify-graph-author
capability to gpt-oss-120b (capabilities.yaml bumped v6 → v7,
role-bindings.yaml bumped v6 → v7), running
`bootstrap_catalog.py --migrate` succeeded but left the
init-seeded v1 binding active alongside v7. The first VGA
dispatch in a fresh workdir failed at iteration 0 with the
uniqueness error.

FT-118 closes the additive gap: when bootstrap writes a new
role binding for role R, it deactivates every other binding
for R *in the same transaction*. The invariant becomes
write-time enforced rather than relying on operator
discipline.

A bonus invariant clarification: the resolver's error
message already names which binding pairs collide, but
operators today have no easy fix (there's no
`dec _deactivate-binding` CLI). FT-118 also exposes a
hidden CLI so the manual recovery path exists if a binding
landed via a different write path and conflicts somehow
arise.

## Functional Specification

### Inputs

- No new operator-facing CLI flags on `bootstrap_catalog.py`.
  The deactivation is internal to the catalog-write
  transaction.
- Hidden CLI for manual recovery:
  - `dec _deactivate-binding --role <role_id> --version
    <version>` — set the named binding's `dec:active=false`.
    Idempotent (already-inactive bindings are no-ops).
  - `dec _activate-binding --role <role_id> --version
    <version>` — symmetric. Errors if activating would
    create a duplicate-active conflict (the invariant
    would-be-violated case).
  - `dec _list-bindings [--role <role_id>]` — print active
    + inactive bindings, optionally filtered. Diagnostic
    aid for understanding catalog state.

### Outputs

- Extension to `dec _bootstrap-catalog` (the hidden command
  the Python `bootstrap_catalog.py` shells out to): the
  SHACL+atomic-transaction write path gains a "deactivate
  prior versions" step for every newly-active role binding
  inserted.
- The deactivation writes
  `<old_binding> dec:active false` quads, replacing the
  `true` ones. Preserves binding history (no quads deleted;
  only flipped).
- New hidden CLI handlers under
  `crates/decision-cli/src/cli/binding_admin.rs`:
  `_deactivate-binding`, `_activate-binding`,
  `_list-bindings`.
- Bootstrap console output extended to surface what was
  deactivated:
  ```
  catalog: 13 capabilities (1 new), 6 role bindings (1 new), 2 deactivated
  deactivated:
    https://decision-cli.dev/ns/binding/verify-graph-author/v1
      (superseded by .../v7 — active since 2026-05-29T15:00Z)
  ```

### State

Persists `dec:active` lifecycle transitions in the
orchestration store. No new persistent files. No new
named-graph layouts.

### Behaviour

1. **Bootstrap insert.** Bootstrap reads the YAML, computes
   the diff against the store, decides which new bindings
   need writing. Standard FT-058 flow.
2. **For each new binding (R, V_new)** about to land as
   active:
   a. SPARQL query: find every `<other_binding>` where
      `dec:roleId = R` AND `dec:active = true` AND
      `<other_binding> != <new_binding>`.
   b. For each match, generate a deactivation patch:
      `<other_binding> dec:active false` (replacing the
      `true` quad).
   c. The deactivations get included in the same
      StreamWriter transaction as the new binding write.
3. **Commit transaction.** New active binding lands;
   former active bindings are flipped to inactive. The
   invariant — exactly one active binding per role —
   holds at every moment a reader observes the store.
4. **Manual recovery CLIs.**
   - `dec _list-bindings --role R` lists every binding
     for R with its version, default capability, active
     flag, and the last-modified timestamp.
   - `dec _deactivate-binding --role R --version V` runs a
     single transaction that flips `<R/V>` to
     `active=false`. No-op if already inactive.
   - `dec _activate-binding --role R --version V` runs a
     single transaction that flips `<R/V>` to
     `active=true` AND deactivates every other binding for
     R in the same transaction (symmetric to bootstrap's
     behaviour). Errors out if no binding `<R/V>` exists.

### Invariants

- **At every observable moment, exactly zero or one active
  binding exists per role.** Zero is acceptable during
  transient operator states (e.g. between
  `_deactivate-binding` and `_activate-binding`) but never
  observable at a single read; the bootstrap transaction
  and the `_activate-binding` transaction both flip
  atomically.
- The historical record is preserved: deactivating a
  binding flips `dec:active` from true to false but
  retains every other triple. Operators can roll back via
  `_activate-binding`.
- The capability resolver's uniqueness check is unchanged;
  this feature ensures the precondition the resolver
  expects.
- Bootstrap's behaviour for new roles (no prior bindings to
  deactivate) is unchanged: the new binding lands active
  with no superseded entries.

### Error handling

- `_activate-binding R V` against an `<R/V>` that doesn't
  exist: exit non-zero with "no such binding". No state
  change.
- `_deactivate-binding R V` against a binding that's
  already inactive: exit zero with "no-op" note.
- A pre-existing store with multiple active bindings
  (i.e. the bug condition we're fixing): the bootstrap
  transaction *first* deactivates ALL bindings for the
  role then inserts the new one as active. So bootstrap
  is the recovery path for stores that hit the bug
  before this feature shipped.
- SHACL validator rejects the deactivation (shouldn't
  happen — `dec:active=false` is a routine state — but
  defence in depth): roll back the whole bootstrap
  transaction, exit non-zero with the SHACL error.

### Boundaries

- This feature does NOT change the resolver's behaviour.
  The resolver still errors on multiple active bindings;
  FT-118 just ensures it doesn't happen.
- This feature does NOT touch capability bindings (the
  capability layer beneath role bindings). Multiple
  capabilities with the same `capability_id` and
  different versions remain expected and allowed —
  capabilities track parallel versions, role bindings
  point at one.
- This feature does NOT modify `bootstrap_catalog.py`'s
  Python shell. The Python script shells out to the
  hidden `dec _bootstrap-catalog`; all logic lives in
  Rust.
- This feature does NOT introduce a "downgrade" CLI
  (re-activating a lower version implicitly). The
  `_activate-binding` command makes the operator
  explicit about which version they want active.

## Out of scope

- **Reactive observation of the resolver error and
  auto-recovery.** The resolver still hard-fails on the
  uniqueness violation; this feature prevents the
  violation, not silent recovery from it.
- **A bulk migration of existing duplicate-active states
  in any historical workdir.** Operators run a fresh
  `bootstrap_catalog.py --migrate` against their store
  to fold this feature's behaviour into it; that's the
  upgrade path.
- **Notifying capability bindings of role-binding
  deactivations.** Capability bindings are upstream of
  role bindings; deactivating a role binding doesn't
  cascade to the capability it points at. (A capability
  with zero active role bindings pointing at it is fine.)
- **Per-environment bindings** (different active bindings
  per BNCH-NNN). All bindings are workdir-global today;
  per-bench scoping is a separate feature.
- **Logging the binding history elsewhere.** The store's
  flipped-active triples are the record; no separate
  ledger.
