# dec-implement-basic fixture

A minimal world for verifying the `dec implement` flow (FT-011 + FT-013 + FT-017) without Claude auth or network egress.

Referenced by `ENV-IMPL-001` (`dec verify env show ENV-IMPL-001`). The runner contract that materialises this tree into a fresh tempdir is documented in [ADR-032 §Runner contract](../../../.product/adrs/ADR-032-verification-fixtures-via-repo-path-reference.md). Until the step executor lands (slice 3+), this fixture is declarative — present so verification graphs can reference it.

## Contents

| Path | Purpose |
|---|---|
| `.product/config.toml` | Minimal product-cli config pointing at the fixture-local feature graph. |
| `.product/features/FT-IMPL-001-*.md` | Trivial feature_spec that `dec implement` can run against. |
| `bin/code-writer` | Shim that forces the real `code-writer` into `CODE_WRITER_STUB=1` mode so the worker emits a deterministic `CodeChange` without calling Claude. Prepended to `$PATH` by the runner contract. |

## Why a fixture-shipped feature_spec instead of the host's

The dec-implement verification needs to exercise `product context FT-XXX` and the implementer dispatch end-to-end (per FT-011 §Behaviour). Pointing at decision-cli's own feature graph would couple the test to whatever FT-NNN happens to be in `in-progress` — flaky. The fixture ships a stable `FT-IMPL-001` so the run is reproducible.

## Why a shim, not a custom stub binary

The real `code-writer` already supports stub mode (`CODE_WRITER_STUB=1`, see `workers/code-writer/src/code_writer/_stub_runner.py`). Re-implementing the dispatch protocol in a one-off binary would drift from the real worker over time. The shim forces stub mode and delegates to the installed worker.
