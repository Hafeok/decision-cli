---
id: TC-113
title: Worker layer has no hardcoded model identifiers outside model_router and test fixtures
type: exit-criteria
status: passing
validates:
  features:
  - FT-064
  adrs:
  - ADR-037
phase: 2
runner: bash
runner-args: tests/scripts/tc-113-no-hardcoded-models.sh
runner-timeout: 60
last-run: 2026-05-23T16:10:19.845721788+00:00
last-run-duration: 1.1s
---

## Description

Invariant: after the migration in [FT-064](FT-064) lands, no hardcoded model identifier or `DEFAULT_MODEL_ID`-shaped constant exists in `workers/` outside of `workers/_shared/src/_shared/model_router.py` (where the router knows how to *call* each endpoint but does not hardcode model ids) and test fixtures (which legitimately pin a specific model for a specific test).

This is the "migration cleanup" assertion from PRD §11.5.

The runner is `bash` driving `grep` against the worker tree, with allowlisted paths for test fixtures.

Acceptance script (`tests/scripts/tc-113-no-hardcoded-models.sh`):

1. **No "claude-sonnet" or "claude-opus" outside fixtures.**
   ```
   grep -rnE 'claude-(sonnet|opus|haiku)' workers/ \
     --include='*.py' \
     --exclude-dir=tests --exclude-dir=__pycache__ --exclude-dir=.venv \
     | grep -v 'workers/_shared/src/_shared/model_router.py'
   ```
   Assert this command returns exit code 1 (no matches).

2. **No "qwen3" / "devstral" / "gpt-oss" / "mistral-small" model ids outside fixtures.**
   Same shape, with these patterns. Catches accidental re-introduction of Scaleway model ids in worker code.

3. **No `DEFAULT_MODEL_ID`-shaped constants.**
   ```
   grep -rnE '^[A-Z_]*MODEL_ID[A-Z_]*\s*=' workers/ \
     --include='*.py' \
     --exclude-dir=tests --exclude-dir=__pycache__ --exclude-dir=.venv
   ```
   Allowlist exception: matches in `model_router.py` are permitted (the router's own endpoint constants). Anywhere else: fail.

4. **No `VERIFIER_MODEL_ID` env-var resolution remains.** `grep -rn 'VERIFIER_MODEL_ID' workers/` returns zero matches.

5. **`anthropic.Anthropic()` construction is centralised.**
   ```
   grep -rn 'anthropic\.Anthropic()' workers/ \
     --include='*.py' \
     --exclude-dir=tests --exclude-dir=__pycache__ --exclude-dir=.venv
   ```
   Assert all matches are inside `workers/_shared/src/_shared/model_router.py`.

6. **Existing TCs still pass.** Run `pytest workers/verifier/tests/ workers/code-writer/tests/` after the migration; assert exit 0 (none of the pre-migration tests for [FT-013](FT-013) / [FT-023](FT-023) are broken — the migration changes plumbing, not behavior).

⟦Σ:Types⟧{
  WorkerSourceTree ≜ set of .py files outside tests/, __pycache__/, .venv/
}

⟦Γ:Invariants⟧{
  ∀ f ∈ WorkerSourceTree, f ≠ model_router.py: hardcoded_model_id ∉ tokens(f)
  ∀ f ∈ WorkerSourceTree, f ≠ model_router.py: 'anthropic.Anthropic()' ∉ tokens(f)
}