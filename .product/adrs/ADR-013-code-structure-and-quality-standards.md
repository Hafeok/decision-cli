---
id: ADR-013
title: Code Structure and Quality Standards
status: accepted
features:
- FT-051
- FT-014
supersedes: []
superseded-by: []
domains: []
scope: platform
content-hash: sha256:dd5e42b86eef90e68bc5191849e46d3fbedae6054d6d9fb11551f95ee921f22c
amendments:
- date: 2026-05-21T11:38:45Z
  reason: 'Drop the exit-2 (warn-band) tier from the fitness-script exit-code contract. The original three-tier design (0=clean / 1=hard / 2=warn) does not compose with product-cli''s test runner, which treats any exit code other than 0/1 as `unrunnable`. Result: TC-016, TC-042, TC-043 sat in `unrunnable` while their underlying scripts ran cleanly with only warn-band diagnostics, blocking phase 1 closure for FT-014 and FT-015. The amendment moves to two-tier (0=clean-or-warn / 1=hard). Warn-band offenders remain visible as `WARNING:` lines on stdout — the soft signal is preserved as diagnostic narrative, just not as a separate exit code. The hard limit still gates CI. A follow-up feature_spec covers the actual shrinking of warn-band offenders.'
  previous-hash: sha256:d42ad20a72f8e2191c5caf67c078bc2cf19f3b477a123d3e017787a59750a3f1
source-files:
- scripts/checks/file-length.sh
---

**Status:** Proposed

**Context:** decision-cli is built for LLM-driven implementation. Two of its three implementation surfaces — the Rust workspace (`crates/oxi-events`, `crates/decision-cli`) and the Python workers (`workers/code-writer`) — already accumulate the failure mode that motivates this decision: files that grow past the point where a context bundle for a single feature can be assembled cleanly.

For human contributors, oversized files indicate poor cohesion. For LLM agents, oversized files are a *context quality* problem. When implementing a feature that touches `subscription.rs`, the agent receives the full file — registry, evaluator, delta computation, transport plumbing — most of which is irrelevant to the change at hand. The agent makes assumptions about the whole file from the parts it can see clearly, producing implementations that are correct in isolation but break adjacent behaviour.

decision-cli has a vocabulary for this problem from its parent framework: a feature with `depth-1-adrs > 8` is a signal to split. A 1600-line source file is the implementation-side equivalent. The same principle applies: bounded scope enables accurate context assembly.

This ADR governs *structural* quality — how the codebase is organised and what limits are enforced. It complements existing rust-side compilation quality (`#![deny(clippy::unwrap_used)]`) and the Stable Dependency Principle constraint on `oxi-events` (no DDD vocabulary, no `decision-cli` imports — see `decision-cli-slice-1-bounds.md` §4.1).

**Decision:** Enforce four structural quality rules with measurable thresholds, checked by scripts in `scripts/checks/` and run via `product verify --platform`. Rules are enforced by auditable shell scripts and a handful of Python equivalents — never by custom lints — so any contributor can read the enforcement and understand exactly what it does.

The same rules apply across both implementation surfaces:

- Rust: every `*.rs` file under `crates/*/src/`.
- Python: every `*.py` file under `workers/*/` (excluding `tests/` and generated stubs).

---

### Rule 1: File Size Limit

No source file may exceed **400 lines** (blank lines and comments included). The 400-line limit is a hard gate — CI fails. A secondary warning threshold of **300 lines** produces a stdout `WARNING:` diagnostic but does not fail CI.

The limit applies to first-party source only. Test files in `crates/*/tests/`, `workers/*/tests/`, and benchmark files in `crates/*/benches/` are exempt — integration scenarios are necessarily verbose.

**Why 400, not 500 or 200?**

200 is too tight for Rust — a module with a substantial type definition, its `impl` blocks, and its error types legitimately reaches 200 lines. 500 is too loose — it permits files that clearly have multiple responsibilities. 400 is the point at which most single-responsibility modules in both Rust and Python fit comfortably.

**Enforcement script** (`scripts/checks/file-length.sh`):

```bash
#!/usr/bin/env bash
# scripts/checks/file-length.sh
# Checks first-party source file lengths in crates/ and workers/.
# Exit 0: every file at or below the hard limit (warn-band offenders, if any,
#         are listed on stdout as advisory diagnostics).
# Exit 1: one or more files exceed the hard limit (400 lines).
set -euo pipefail

HARD_LIMIT=${FILE_LENGTH_HARD:-400}
WARN_LIMIT=${FILE_LENGTH_WARN:-300}

# Rust under crates/<crate>/src ; Python under workers/<worker>/ (excluding tests)
FILES=$( { \
  find crates -path '*/src/*.rs' 2>/dev/null; \
  find workers -name '*.py' -not -path '*/tests/*' 2>/dev/null; \
  } | sort -u )

[ -z "$FILES" ] && { echo "OK: no first-party source files found"; exit 0; }

HARD_VIOLATIONS=$(echo "$FILES" | xargs wc -l \
  | awk -v limit="$HARD_LIMIT" '$1 > limit && $2 != "total" {print $1, $2}' \
  | sort -rn)

WARN_VIOLATIONS=$(echo "$FILES" | xargs wc -l \
  | awk -v wl="$WARN_LIMIT" -v hl="$HARD_LIMIT" \
    '$1 > wl && $1 <= hl && $2 != "total" {print $1, $2}' \
  | sort -rn)

if [ -n "$HARD_VIOLATIONS" ]; then
  echo "ERROR: files exceeding hard limit ($HARD_LIMIT lines):"
  echo "$HARD_VIOLATIONS" | while read -r count file; do
    echo "  $file: $count lines (limit: $HARD_LIMIT)"
  done
  exit 1
fi

if [ -n "$WARN_VIOLATIONS" ]; then
  echo "WARNING: files approaching limit ($WARN_LIMIT–$HARD_LIMIT lines):"
  echo "$WARN_VIOLATIONS" | while read -r count file; do
    echo "  $file: $count lines (warn at: $WARN_LIMIT)"
  done
fi

echo "OK: all source files within hard limit"
exit 0
```

---

### Rule 2: Function Length Limit

No function body may exceed **40 lines** (blank lines excluded — only statement lines count). Trait `impl` blocks (Rust) and class bodies (Python) may be longer, but each individual method within them must respect the 40-line limit.

**Why 40?** A function that exceeds 40 statement lines is almost always doing more than one thing. The remedy is always the same: name the sub-operation and extract it. The name is documentation. The extraction is a seam for testing.

Enforcement is per-language:

- Rust: `scripts/checks/function-length.sh` — awk-based detection of `fn` headers and brace-depth tracking.
- Python: `scripts/checks/function-length.py` — uses the `ast` module, counts statement nodes within each `FunctionDef` / `AsyncFunctionDef`.

Both scripts share the same threshold envelopes (`FN_LENGTH_HARD=40`, `FN_LENGTH_WARN=30`) and the same two-tier exit code semantics as Rule 1: exit 1 when any function body exceeds the hard limit; exit 0 otherwise. Warn-band offenders (31–40 statement lines) are listed on stdout as `WARNING:` diagnostics but do not gate CI.

---

### Rule 3: Module Decomposition

Each top-level module under `crates/*/src/` and `workers/*/` declares a single stated responsibility. Cross-module imports may go only through the public surface (`mod.rs` for Rust, `__init__.py` for Python) — not into a sibling module's internals.

decision-cli's canonical structure inherits the slice 1 split (see `decision-cli-slice-1-bounds.md` §5):

```
crates/
  oxi-events/src/
    lib.rs            # crate root — re-exports
    writer.rs         # GraphWriter mutation chokepoint
    subscription.rs   # Subscription type + registry
    evaluator.rs      # subscription evaluator
    event.rs          # Event type and outbox flag
    outbox.rs         # outbox publisher background task
    transport/        # delivery transports
      mod.rs
      broadcast.rs    # in-process tokio broadcast
      sse.rs          # axum SSE
    replay.rs         # SPARQL-based replay
  decision-cli/src/
    main.rs           # clap entry point only — dispatch, no logic
    error.rs          # decision-cli error type
    config.rs         # .dec/ store config
    ontology/         # embedded ontology + SHACL shapes
    streams/          # ValueStream and ValueAction definitions
    init/             # dec init: parse, validate, resolve, persist
    store/            # orchestration Oxigraph store
    dispatch/         # role dispatch protocol
    session/          # session records, PROV-O linkage
    product/          # subprocess invocation of product-cli
    commands/         # one file per command group, no logic
workers/
  code-writer/
    code_writer/      # package
      __init__.py
      bundle.py       # bundle parsing
      worker.py       # Claude dispatch
      output.py       # CodeChange artifact construction
```

`main.rs` must contain only: the `clap` derive macro, the top-level `match` dispatching to `commands/`, and the call to `std::process::exit`. No logic. If `main.rs` exceeds 80 lines, it is a violation.

A separate script (`scripts/checks/module-structure.sh`) asserts the presence of the required top-level modules in each crate and the line-cap on `main.rs`.

---

### Rule 4: Single Responsibility Naming Contract

Each source file must begin with a one-sentence responsibility comment. The sentence must not contain "and" — if it does, the file has two responsibilities and must be split.

- Rust uses `//!` module doc comments:

```rust
//! Single chokepoint for graph mutations — produces typed events on commit.

//! Brandes' betweenness centrality over the decision graph.
```

- Python uses module docstrings:

```python
"""Parses an incoming dispatch bundle into a CodeWriterInput struct."""
```

Checked by `scripts/checks/single-responsibility.sh`. The script is straightforward: the first non-shebang line of each first-party source file must match `^//! ` (Rust) or `^"""` (Python), and must not contain the substring " and " (with surrounding spaces).

---

### Rule scope and the oxi-events SDP boundary

The rules apply uniformly — but enforcement honors the Stable Dependency Principle on `crates/oxi-events`. The single-responsibility rule, in particular, prohibits responsibility comments in `oxi-events` from naming DDD-specific concepts (roles, bundles, sessions, policies, model bindings, autonomy levels). A doc comment like `//! Dispatches roles via the subscription evaluator.` in `oxi-events` is a violation independent of the "and" check — it imports application vocabulary that the crate must not know about. This is enforced by reading `decision-cli-slice-1-bounds.md` §4.1; mechanical enforcement (a forbidden-words check) is deferred.

---

### Exit-code contract for fitness scripts

All ADR-013 enforcement scripts use a uniform **two-tier** exit-code contract so they compose cleanly with product-cli's binary pass/fail test runner:

- **Exit 0** — clean tree, or warn-band offenders only. Any warn-band offenders are emitted on stdout as `WARNING:` diagnostic lines that name the file, line, and offending count. The diagnostics are advisory: they surface drift toward the hard limit without gating CI.
- **Exit 1** — at least one hard-limit violation. Diagnostic lines on stdout enumerate the offenders.

An earlier revision of this ADR specified a three-tier contract (exit 2 = warn). That tier was removed because product-cli's test runner treats any exit code other than 0 or 1 as `unrunnable`, which left fitness TCs stuck in a stale state whenever the tree had any warn-band drift. The two-tier contract preserves the soft signal (the `WARNING:` lines remain) but routes it through narrative rather than exit code.

---

### TC Files

The cross-cutting TCs that validate these rules carry `scope: cross-cutting` (per the parent product-cli framework's classification — these TCs validate every feature's implementation implicitly). They run via `product verify --platform`. Each TC uses `runner: bash` (or `runner: pytest` where the underlying script is Python) pointing to the enforcement scripts.

Names follow the convention `TC-CQ-NNN` for traceability across the workspace, but they take ordinary `TC-NNN` IDs in the graph.

---

### Integration with `product verify --platform`

The TCs that validate Rules 1–4 have empty `validates.features` — they are not linked to a specific feature. They are validated via `product verify --platform`, which runs all TCs linked to cross-cutting ADRs. This ADR is `scope: cross-cutting`.

Consequence: every time any feature in decision-cli is implemented and `product verify --platform` is run, the code-quality checks run alongside the platform invariants. A new file that creeps past 400 lines fails the platform check, not just a code review comment. The same check covers both the Rust workspace and the Python workers in a single invocation.

---

**Rationale:**

- File size limits are not aesthetic. For LLM-driven development they are a *context quality* constraint. A 1600-line file means the implementation agent receives 1600 lines when it needs 80. The agent either truncates (missing context) or processes everything (noise drowning signal). Both outcomes produce worse implementations than a focused 200-line file.
- The single-responsibility doc comment rule is self-enforcing documentation. Writing `//! Graph traversal and centrality computation.` and seeing it fail CI because of "and" is a clearer signal than a code review comment saying "this file has two responsibilities."
- Shell scripts (with a small amount of Python `ast` for function-length on Python sources) make the rules auditable. Any developer can read `file-length.sh` and understand exactly what it checks. A custom clippy lint requires understanding `rustc`'s plugin API. Shell scripts and `ast` are boring and correct.
- The 400-line hard limit with a 300-line warning gives two signals: "you're approaching the limit" (warning, visible on stdout) and "you've exceeded it" (error, blocks CI). The warning is the more valuable signal — it's caught before the file becomes a problem.
- Applying the same rules to both Rust and Python keeps the harness coherent. decision-cli is heterogeneous on purpose (the Rust binary owns orchestration; the Python workers own LLM calls). The quality contract is uniform so the heterogeneity does not produce drift.

**Rejected alternatives:**

- **Rust-only enforcement** — would leave the Python workers as a hidden growth surface. Rejected: the worker contract is part of the system. If the worker code drifts past 1000-line files, the implementer role degrades in exactly the way this ADR is designed to prevent.
- **Custom clippy lint for file length** — requires understanding `rustc`'s internal span API. Brittle across Rust versions. Rejected: shell script is simpler, more portable, and more readable.
- **tokei / pylint plugin** — external dependencies. Rejected: `wc -l`, `awk`, and Python's standard-library `ast` are universally available.
- **250-line limit** — too tight for legitimate cases like the Brandes centrality implementation referenced by ADR-029 in product-cli. Rejected.
- **No module structure mandate** — leaves module decomposition to the implementing agent's judgment. Agents without a defined module structure will make different choices on different features, producing inconsistent organisation that compounds over time. A defined structure (Rust modules per the slice 1 split; Python packages per worker) eliminates this decision entirely.
- **Three-tier exit codes (clean / warn / hard)** — the original revision of this ADR defined exit 2 as a warn-band signal. The product-cli test runner only distinguishes exit 0 (pass) from exit 1 (fail); any other code lands as `unrunnable`. Rejected in amendment: the runner contract is binary, so the warn-band signal moves to stdout diagnostics.

**Derivation:** This decision is adapted directly from product-cli ADR-029 ("Code Structure and Quality Standards"). The thresholds (400/300, 40/30, 80-line `main.rs`) are inherited unchanged. The two additions are: (1) Python coverage for the workers, and (2) explicit interaction with the Stable Dependency Principle constraint on `oxi-events`.