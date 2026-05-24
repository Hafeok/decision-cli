---
id: TC-017
title: every_cross_cutting_adr_is_backed_by_a_runner_tc
type: invariant
status: passing
validates:
  features: []
  adrs:
  - ADR-014
phase: 1
runner: bash
runner-args: scripts/checks/cross-cutting-rules-have-checks.sh
runner-timeout: 60
last-run: 2026-05-24T19:14:23.673322616+00:00
last-run-duration: 5.6s
---

## Purpose

Self-check of the **ADR-014** convention: every cross-cutting ADR in
`.product/adrs/` that codifies a mechanically checkable rule should be
backed by at least one TC whose front-matter (a) lists the ADR in
`validates.adrs` and (b) declares a non-empty `runner`. Without that
pairing, the rule lives in the bundle but `product verify --platform`
has nothing to run for it.

Exit code semantics intentionally use **warning (2)** rather than hard
failure (1): pre-existing cross-cutting ADRs may legitimately have no
mechanical check (e.g. ADR-001's SDP boundary is enforced by code review).
The primary enforcement for *new* rule ADRs missing a runner TC is the
product-cli workflow gate (FT-058) which blocks the feature from
transitioning to in-progress. This TC is a backstop — it surfaces drift,
it does not self-veto the merge that introduces a new rule.

This TC has empty `validates.features` by design: per ADR-014, rule TCs
are cross-cutting.

## Given

- A working copy of decision-cli with a populated `.product/adrs/` and
  `.product/tests/`.
- `bash`, `awk`, and `grep` available on `PATH`.

## When

```bash
scripts/checks/cross-cutting-rules-have-checks.sh
```

## Then

1. Exit 0 if every ADR with `scope: cross-cutting` **and**
   `status: accepted` has at least one TC whose front-matter lists the
   ADR id under `validates.adrs` and whose `runner` field is non-empty.
2. Exit 2 (warning) if at least one accepted cross-cutting ADR has no
   such TC. Diagnostic lines on stdout name each ADR id with no runner
   TC.

ADRs in `status: proposed`, `superseded`, or `abandoned` are skipped:
proposed ADRs are design documents in flight (the workflow gate in
product-cli — FT-058 — runs the runner-TC check before a feature can
transition to in-progress, which is where new rules become binding);
superseded / abandoned ADRs no longer require enforcement.

## Notes

- TC-CQ-META in the workspace narrative; TC-017 in the graph.
- The pairing is one-way (TC → ADR via `validates.adrs`). The reverse
  pointer (ADR → TC) is derived by product-cli's graph walker; the script
  reads only the TC side.
- The convention assumes one source of truth for "is this a mechanical
  rule" — namely, whether the author chose to pair the ADR with a runner
  TC. Non-mechanical cross-cutting ADRs accept the warning and move on.

## Formal specification

⟦Σ:Types⟧{
  Adr ≜ ⟨id:IRI, scope:Scope, status:Status, source:Path⟩
  Tc ≜ ⟨id:IRI, validates_adrs:Set[IRI], runner:String?, source:Path⟩
  Scope ≜ feature-specific | cross-cutting
  Status ≜ proposed | accepted | superseded | abandoned
  BindingAdrs ≜ {a:Adr | a.scope = cross-cutting ∧ a.status = accepted}
  RunnerTcs ≜ {t:Tc | defined(t.runner) ∧ t.runner ≠ ""}
}

⟦Γ:Invariants⟧{
  ∀a:BindingAdrs:
    ∃t:RunnerTcs: a.id ∈ t.validates_adrs
  ¬(∃a:BindingAdrs: ∄t:RunnerTcs: a.id ∈ t.validates_adrs)
    ⇒ exit_code = 2  -- warning, not block, per ADR-014 §Enforcement
}

⟦Ε⟧⟨δ≜0.85;φ≜90;τ≜◊⁺⟩