---
id: TC-029
title: VerificationVerdict conforms to ADR-018 SHACL shape
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
runner-args: --package decision-cli --test verdict_shacl
runner-timeout: 180
last-run: 2026-05-21T13:31:41.447481757+00:00
last-run-duration: 1.1s
---

## Description

Invariant: every persisted `dec:VerificationVerdict` artifact conforms to the SHACL shape defined in [ADR-018](ADR-018): `dec:VerificationVerdictShape`. The shape constrains:

- `dec:kind` ∈ {`approved`, `amendment-required`, `rejected`} (one value, required).
- `dec:actionSessionId` and `dec:interpretationSessionId` are present and reference existing sessions.
- `dec:cites` is non-empty when `kind ≠ approved`.
- `dcterms:created` is a valid `xsd:dateTime` literal.

The runner is a `cargo-test` integration that loads a sample of persisted verdicts plus a battery of constructed-invalid verdicts, runs `oxigraph::shacl::validate`, and asserts the valid ones pass and the invalid ones produce specific shape-violation reports.

⟦Σ:Types⟧{
  Verdict ≜ ⟨kind:VerdictKind, actionId:IRI, interpretationId:IRI, cites:CitationSet, created:DateTime⟩
  VerdictKind ≜ approved | amendment-required | rejected
}

⟦Γ:Invariants⟧{
  ∀ v:Verdict: shacl_conforms(v, VerificationVerdictShape)
  ∀ v:Verdict: v.kind ≠ approved ⇒ |v.cites| ≥ 1
}