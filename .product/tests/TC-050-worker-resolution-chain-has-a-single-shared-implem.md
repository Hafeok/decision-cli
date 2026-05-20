---
id: TC-050
title: worker resolution chain has a single shared implementation
type: invariant
status: unimplemented
validates:
  features:
  - FT-016
  adrs: []
phase: 1
runner: bash
runner-args: scripts/checks/worker-resolution-single-source.sh
runner-timeout: 30
---

## Description

FT-016 promises a single source of truth for worker resolution: a `worker::resolve` module consumed by both `dec implement` and `dec doctor`. Two copies of the chain (one inside `implement.rs`, one inside `doctor.rs`) would silently drift and re-introduce exactly the gap FT-016 closed. This invariant is a structural rule: there is exactly one definition of the chain, and every consumer routes through it.

## Acceptance Criteria

Given the decision-cli workspace at any commit:

1. **Single definition.** A repository-wide grep for the signature of the resolution entry point (e.g. `fn resolve(role: &Role) -> Resolution`) finds exactly one match, inside the `worker` module of `crates/decision-cli`. Multiple matches fail the invariant.

2. **Implement consumes the shared API.** The body of the `dec implement` worker invocation (today `run_worker` in `crates/decision-cli/src/implement/worker.rs`) reaches a worker invocation only via a `worker::resolve(...)` call — no inline `which(...)`, no inline `std::env::var("CODE_WRITER_CMD")`, no inline `python3 -c "import ..."` probe outside the shared module.

3. **Doctor consumes the shared API.** The `dec doctor` command handler reaches the audit only via `worker::resolve(...)`.

4. **No private duplicates.** No file under `crates/decision-cli/src/` (other than the canonical `worker::resolve` module) contains the string `python3 -c "import code_writer.main"` or its slice-1 equivalent. New roles, when they arrive, extend the manifest — not the call sites.

## Runner

Mechanical enforcement is by `scripts/checks/worker-resolution-single-source.sh`. The script:

- `rg -n 'fn resolve\b' crates/decision-cli/src/worker/` — must produce ≥1 hit
- `rg -n 'fn resolve\b' crates/decision-cli/src/ --glob '!worker/'` — must produce 0 hits
- `rg -n '"CODE_WRITER_CMD"' crates/decision-cli/src/ --glob '!worker/'` — must produce 0 hits
- `rg -n 'python3 -c "import code_writer' crates/decision-cli/src/ --glob '!worker/'` — must produce 0 hits

Exit 0 on clean tree, exit 1 on any violation with offending lines on stdout.

## Formal specification

⟦Σ:Types⟧{
  Module ≜ ⟨path:Path, syms:Set⟨Ident⟩⟩
  ResolveDef ≜ {m:Module, name:Ident | name = "resolve" ∧ has_fn(m, name)}
  WorkerProbe ≜ {call:CallSite | call.target ∈ {which, env_var("CODE_WRITER_CMD"), python_import_probe}}
}

⟦Γ:Invariants⟧{
  |{r:ResolveDef | r.m.path ⊆ "crates/decision-cli/src/worker/"}| = 1
  |{r:ResolveDef | ¬(r.m.path ⊆ "crates/decision-cli/src/worker/")}| = 0
  ∀p:WorkerProbe: p.module.path ⊆ "crates/decision-cli/src/worker/"
}

⟦Ε⟧⟨δ≜1.0;φ≜100;τ≜◊⁺⟩
