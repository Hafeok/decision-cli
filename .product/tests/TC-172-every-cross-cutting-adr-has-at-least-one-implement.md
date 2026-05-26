---
id: TC-172
title: Every cross-cutting ADR has at least one implementing feature linked, modulo documented exclusions
type: exit-criteria
status: unimplemented
validates:
  features:
  - FT-103
  adrs: []
phase: 1
---

## Claim

Every **accepted** cross-cutting ADR in the product graph has at least one feature in its `features:` frontmatter list, **except** for ADRs on a documented exclusion or delegation list. The assertion runs as a structural check over the entire `.product/` tree, not against any single feature's preflight.

## Scenarios

### Setup

- A fully-loaded product graph (the live `.product/` tree at TC execution time).
- The exclusion + delegation lists captured in FT-103's body, re-stated here for self-containment:
  - **Excluded** (forward-looking / cross-stream — no decision-cli implementer expected):
    - `ADR-065` (Dagger deferred as worker runtime model).
    - `ADR-044` (Brief as a typed artifact in product-cli's catalog — implemented in product-cli, not here).
  - **Delegated** (will be satisfied by the platform fitness-functions feature, not by per-slice implementer):
    - `ADR-014` (fitness functions tracked as product-cli artifacts).
    - `ADR-021` (action-interpretation agreement as fitness metric).

### Scenario A — happy path: every applicable cross-cutting ADR has ≥1 implementer

Run a SPARQL query against the product graph:

```sparql
PREFIX prod: <https://product-cli.dev/ns#>

SELECT ?adr WHERE {
  ?adr a prod:ADR ;
       prod:status "accepted" ;
       prod:scope "cross-cutting" .
  FILTER NOT EXISTS { ?adr prod:features ?f }
  FILTER (?adr NOT IN (<ADR-065>, <ADR-044>, <ADR-014>, <ADR-021>))
}
```

Expected result: **empty result set** — no cross-cutting accepted ADR is without an implementer feature (excluding the documented set).

If the query returns ≥1 row, the test fails with output `Cross-cutting ADR <ADR-NNN> has no implementing feature. Either link the implementing feature (product feature link FT-NNN --adr ADR-NNN) or add ADR-NNN to FT-103's exclusion/delegation list with a justification.`

### Scenario B — exclusion list is closed

A separate assertion: the test's hardcoded exclusion + delegation set (the four IDs above) must match FT-103's body verbatim. The test parses FT-103's markdown for the exclusion / delegation lists and compares set equality. Drift → fail with `FT-103 exclusion list and TC-172 exclusion set out of sync; one was updated without the other.`

This keeps the rationale in the feature body in sync with the executable assertion; either both move or the test fails fast.

### Scenario C — superseded ADRs are not counted

Add a fixture (or check live state) where `ADR-X` is `superseded` rather than `accepted`. Assertion: the SPARQL query's `prod:status "accepted"` filter excludes it, so superseded ADRs without implementers do not fail the test. (Superseded ADRs are historical records; the active set is what matters for cross-cutting coverage.)

### Scenario D — the SPARQL query's namespace assumptions are stable

A sub-assertion that the predicate names used in the query (`prod:status`, `prod:scope`, `prod:features`) match the actual product-cli graph schema. Discovered via `product schema --format turtle` or equivalent; the test must update if product-cli renames the predicates. (This is the "test the test" pattern — keeps it from silently passing because the query found zero rows for the wrong reason.)

### Scenario E — `scope: cross-cutting` is the canonical marker

The test only inspects ADRs whose frontmatter declares `scope: cross-cutting` (or equivalent — the exact frontmatter key may be `scope` or a synonym; the test pins it). ADRs with `scope: domain` or `scope: deferred` are out of the test's scope by construction. This means the **re-scope sweep** (option 3 in the original three-pronged plan) is the right escape valve for ADRs that shouldn't have been cross-cutting in the first place: re-scoping clears the gap without the link backfill.

## Runner

`bash tests/scripts/tc-172-cross-cutting-adr-coverage.sh`. The script:

1. Runs the SPARQL query above against the product-cli graph (via `product graph sparql --query <inline> --format json`, or via `python -c "import oxigraph; ..."` if product-cli does not surface a SPARQL endpoint).
2. Parses FT-103's body to extract the exclusion + delegation list.
3. Asserts (a) query returns empty, (b) exclusion set matches FT-103.
4. Exits 0 if both pass; exits 1 with the diagnostic above otherwise.

Depends on product-cli exposing a SPARQL query verb — if not yet available, the test ships a Python fallback that loads every `.product/adrs/*.md` frontmatter directly and computes the assertion in-process. Either path is acceptable; the *contract* is the assertion, not the mechanism.

## Non-goals

- Verifying that each linked feature's implementation is **correct** (FT-103 explicitly scopes this out — the link is a structural claim, not a substantive one).
- Cross-stream coverage (e.g. whether `ADR-044` is linked from product-cli's own graph — that's product-cli's TC, not ours).
- Per-feature cross-cutting checks (the existing `product preflight FT-XXX` cross-cutting check stays in place; this TC is the **complementary** system-wide check that the gaps surfaced per-feature are real, not just unlinked).
- Drift detection between an ADR's stated intent and its implementer's actual code (a deeper concern, out of slice; the `product drift check` verb already exists for the spec-vs-code dimension).
