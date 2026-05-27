---
id: TC-030
title: rejected verdict cites at least one TC or ADR
type: invariant
status: passing
validates:
  features:
  - FT-020
  - FT-023
  adrs:
  - ADR-018
phase: 2
runner: cargo-test
runner-args: --package decision-cli --test verdict_rejected_cites
runner-timeout: 120
last-run: 2026-05-27T13:12:20.957136593+00:00
last-run-duration: 0.2s
---

## Description

Invariant: every `dec:VerificationVerdict` with `kind = "rejected"` carries at least one citation in `dec:cites` pointing to a `dec:TC` or an `ADR` artifact. The intent ([ADR-018](ADR-018)) is that rejection is *grounded* in a concrete acceptance criterion or architectural decision; an ungrounded rejection is non-actionable and indistinguishable from an opinion.

## Runner

```sparql
PREFIX dec: <https://decision-cli.dev/ns/>
ASK WHERE {
  ?v a dec:VerificationVerdict ; dec:kind "rejected" .
  FILTER NOT EXISTS {
    ?v dec:cites ?c .
    { ?c a dec:TC } UNION { ?c a dec:ADR }
  }
}
```

If the ASK returns `true`, at least one rejected verdict is ungrounded; runner exits 1.

⟦Γ:Invariants⟧{
  ∀ v:Verdict: v.kind = rejected ⇒
    ∃ c ∈ v.cites: type(c) = TC ∨ type(c) = ADR
}