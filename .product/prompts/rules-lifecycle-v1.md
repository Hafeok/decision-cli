# Authoring a new rule under ADR-014 — worked example

You are landing a new code-quality rule (or other architectural fitness
function) in decision-cli. Rules live as cross-cutting ADRs in the
internal `.product/` graph; their mechanical checks live as TCs with
runners. The convention is governed by ADR-014. This prompt is the
worked example FT-015 ships.

## Before you start

1. `product graph central` — read the top cross-cutting ADRs to understand
   what rules already exist.
2. `product adr list --status accepted` — scan the rule surface to avoid
   restating an existing rule.
3. Decide whether the rule is mechanically checkable (script can answer
   yes/no) or only review-checkable (no script possible). Only mechanical
   rules need a runner TC; non-mechanical rules still ship as ADRs but
   produce a warning in `cross-cutting-rules-have-checks.sh`.

## Five-step flow

### 1. Author the ADR

```bash
product adr new "Rule title — what it forbids in one phrase"
product adr scope ADR-NNN cross-cutting        # mandatory for a rule
product adr domain ADR-NNN --add <domain>      # at least one from .product/config.toml
```

Edit the ADR body with:
- the rule statement (what is forbidden, with what threshold),
- the rationale (why this rule, with what trade-off considered),
- the enforcement script path (`scripts/checks/<rule>.sh`),
- the rejected alternatives section.

### 2. Write the enforcement script

Land `scripts/checks/<rule>.sh`. Contract:

- Exit 0 — clean.
- Exit 1 — hard violation (blocks merge under ADR-014).
- Exit 2 — warning band (allows merge, surfaces in PR comment).
- Diagnostics on stdout (`product verify` captures these).
- Script-self errors on stderr.
- Dependencies limited to what is universally available on a CI runner
  (`bash`, `awk`, `find`, `wc`, Python 3 stdlib).

Make it executable: `chmod +x scripts/checks/<rule>.sh`.

### 3. Author the TC

```bash
product test new "<rule_check_name>" --type invariant
product test runner TC-NNN --runner bash \
  --args scripts/checks/<rule>.sh --timeout 60s
```

Edit the TC front-matter so that:

```yaml
validates:
  features: []          # empty — rules are cross-cutting, not feature-tied
  adrs:
  - ADR-NNN             # the rule ADR you just authored
status: implemented     # not "unimplemented" — the script is the implementation
```

Fill in `## Purpose`, `## Given`, `## When`, `## Then` per the existing TCs
(see TC-016 and TC-017 for templates).

### 4. Couple the script to the ADR

```bash
product adr source-files ADR-NNN --add scripts/checks/<rule>.sh
```

This wires drift detection. `product drift check ADR-NNN` will surface
changes to the script that are not paired with an ADR amendment.

### 5. Apply atomically and verify

```bash
product request apply              # if authored through `product request`
product graph check                # structural validation
product verify --platform          # runs your TC alongside the others
```

CI on the PR runs the same `product verify --platform`. The per-script
classification step in `.github/workflows/product-verify-platform.yml`
distinguishes block (exit 1) from warning (exit 2) per ADR-014.

## Common mistakes

- **Forgetting `scope: cross-cutting`.** The rule won't appear in any
  feature's context bundle. `product graph check` should warn (W010 etc.).
- **Forgetting `validates.adrs` on the TC.** The TC won't run under
  `product verify --platform`. `cross-cutting-rules-have-checks.sh` (TC-017)
  will surface this as a warning.
- **Hard-failing the script (exit 1) on a soft signal.** ADR-014 reserves
  exit 1 for things that should block the merge. Anything weaker is exit 2.
- **Naming a domain not in `.product/config.toml`.** `product request apply`
  rejects the ADR with a validation error. Either pick an existing domain
  or amend `config.toml` in the same request.

## Cross-references

- [ADR-014 — Architectural Fitness Functions Tracked as product-cli Artifacts](../adrs/ADR-014-architectural-fitness-functions-tracked-as-product.md)
- [FT-015 — Use the Internal product-cli Graph as the Source of Truth for Code Quality Rules](../features/FT-015-use-the-internal-product-cli-graph-as-the-source-o.md)
- [ADR-013 — Code Structure and Quality Standards](../adrs/ADR-013-code-structure-and-quality-standards.md) (the first rule under this convention)
- [TC-016](../tests/TC-016-source-file-length-within-adr-013-limits.md) and [TC-017](../tests/TC-017-every-cross-cutting-adr-is-backed-by-a-runner-tc.md) (the first cross-cutting TCs)
- [`scripts/checks/file-length.sh`](../../scripts/checks/file-length.sh) and [`scripts/checks/cross-cutting-rules-have-checks.sh`](../../scripts/checks/cross-cutting-rules-have-checks.sh) (the first enforcement scripts)
