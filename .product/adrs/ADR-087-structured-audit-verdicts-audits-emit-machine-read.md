---
id: ADR-087
title: 'Structured audit verdicts: audits emit machine-readable per-check results; repair targeting consumes the verdict, not parsed text'
status: accepted
features: []
supersedes: []
superseded-by: []
domains:
- data-model
- error-handling
- observability
scope: cross-cutting
content-hash: sha256:455c64a0f2bd41d9c711be71c57844bf037f945ee97ceca85dfff6572e65909a
---

**Status:** Proposed

## Context

[ADR-080](ADR-080) gave every TaskType a coherence audit; [FT-171](FT-171) added the audit-repair loop that re-dispatches only the cells implicated by a failed audit; [FT-172](FT-172) added expensive checks (compile probe, canonical namespace) to the `add-artifact-type` audit. The contract between an audit and the repair loop, however, is **prose**: the audit script emits `FAIL check=<name>: <detail>` lines on stdout, and `cluster_dispatch.rs` (`implicated_cells()`, ~lines 956–1005) extracts file paths and check names from that text, joins them against cell `output_path`s and a hardcoded check→cell ownership table, and — when nothing matches — degrades to re-dispatching **every** cell (witnessed by TC-435).

Three weaknesses follow from parsing prose:

1. **Targeting precision depends on wording discipline.** An audit author who phrases a failure without a recognisable `file:line` or `check=` token silently downgrades repair from "one cell" to "all cells", multiplying repair token cost by cluster width. Nothing in the type system catches the regression; the audit still exits 1 correctly.
2. **The verdict is not graph-resident.** The cluster SessionRecord ([FT-146](FT-146)) records per-cell token counts and a pass/fail status, but not *which checks failed with what detail*. `dec session show` can say a cluster failed; the operator must re-run the audit by hand to learn why. Round-over-round repair history (`dec drive show`, [FT-113](FT-113)) cannot explain why a given cell was re-dispatched.
3. **Every new audit re-negotiates the convention.** The FT-172 compile probe had to specify, in its feature body, which cells its failures map to ("namespace → `iri_module_consts`/`shacl_shape`; compile → all Rust cells"). Each future audit — including the archetype and seam audits mandated by [ADR-082](ADR-082)/[ADR-084](ADR-084) — repeats this negotiation in prose, and the mapping lives in Rust match arms rather than in the audit's own declaration.

The pipeline factory (`~/projects/pipeline`) solved the same problem with typed gates: every step's output passes through a pure gate function that returns a structured `GateResult` — named per-check pass/fail with a message per check (`src/domain/gates/build-gate.ts`) — and the retry prompt is built mechanically from the failed checks (`run-step.ts` `buildRetryPrompt`). The gate→retry contract is a type, not a parse. We borrow that principle and adapt it to our cell-cluster shape: checks must additionally declare *which cells they implicate*, because our retry unit is a cell, not a whole step.

[ADR-013](ADR-013)'s two-tier runner contract (exit 0 pass / 1 fail / 2 unrunnable) is unaffected: the exit code remains the gate, the verdict becomes its payload.

## Decision

**Every cluster audit emits a machine-readable verdict document alongside its human-readable diagnostics. The harness consumes the verdict — never parsed prose — for repair targeting, retry context, and graph-resident session history.**

1. **Verdict contract (`dec-audit-verdict/v1`).** An audit script writes a JSON document to `<sandbox>/.dec-audit-verdict.json` (path supplied by the harness via env var `DEC_AUDIT_VERDICT_PATH`):

   ```json
   {
     "schema": "dec-audit-verdict/v1",
     "audit": "cluster-audit-add-artifact-type",
     "checks": [
       {
         "name": "compile_probe",
         "status": "fail",
         "detail": "error[E0277]: `f32` doesn't implement `Eq` ...",
         "implicates": { "cells": ["struct_module"], "files": ["crates/dec-ontology/src/archetype.rs"] }
       }
     ]
   }
   ```

   `status` is `pass | fail | unrunnable` per check. `implicates.cells` names cells directly where the check knows them; `implicates.files` lists sandbox-relative paths that the **harness** maps to owning cells via the cells' declared `output_path`s ([FT-166](FT-166), [FT-170](FT-170)) — a deterministic join, not a regex over prose.
2. **The harness owns the handshake.** `CoherenceAuditSpec` ([FT-139](FT-139)) gains a `verdict: v1 | legacy-text` field. For `v1` audits the harness sets `DEC_AUDIT_VERDICT_PATH` before invoking the script and reads the document after exit; a `v1` audit that exits 1 without writing a parseable verdict is `unrunnable` (exit-2 semantics), never a silent pass.
3. **Repair targeting consumes the verdict.** The implicated set for a repair round is the union of (a) cells named directly by failed checks and (b) cells owning the files named by failed checks. Only a failed check with an empty `implicates` degrades to the all-cells fallback — and the degradation is recorded on the session record as the reason for the broad re-dispatch.
4. **Retry context is built from the verdict.** A re-dispatched cell receives only the failed checks that implicate it (name + detail), replacing the current whole-diagnostic block. Corrective context narrows with targeting.
5. **The verdict persists to the graph.** The cluster SessionRecord mutation ([FT-146](FT-146)) gains per-check quads: check name, status, detail (truncated), implicated cells, and whether targeting degraded. `dec session show` renders the audit narrative; `dec drive show` explains each repair round from the graph alone.
6. **Grandfathering.** An audit declared `verdict: legacy-text` keeps today's text extraction, and every failed run records `verdict-noncompliant` on the session record. Existing audits migrate as they are next touched; new audits MUST declare `v1` (the audit-conventions check enforces it).

## Rationale

- **Deterministic targeting.** The repair loop's precision becomes a property of declared data (check → cells, file → output_path) rather than of diagnostic prose style. An audit cannot silently regress repair to the all-cells path by rewording a message.
- **Graph-resident observability.** Why a cluster failed, which checks failed, and why specific cells were re-dispatched become queryable facts, consistent with the graph-as-state principle ([ADR-003](ADR-003)). The operator debugging a failed cluster reads `dec session show`, not raw audit stdout.
- **A contract new audits inherit.** The archetype-conformance and seam audits required by [ADR-082](ADR-082)/[ADR-084](ADR-084) land on a typed contract instead of each negotiating a text convention. For seam audits specifically, the persisted per-check verdict doubles as the evidence record the monolith bar requires (`monolith_bar_evidence`, ADR-084 §2).
- **Borrowed where it is strong, adapted where we differ.** The factory's GateResult proves the typed-gate principle in production; our adaptation adds cell implication because our retry unit is finer than a step.

## Rejected alternatives

- **Harden the text-parsing instead.** Stricter regexes over `FAIL check=...` lines keep the convention implicit, keep the mapping in Rust match arms, and provide nothing to persist. The failure mode (silent degradation to all-cells) remains.
- **One script per check, exit codes only.** Mirroring product-cli's one-runner-per-TC shape would make each check independently runnable but loses shared expensive setup (the FT-172 compile-probe worktree), multiplies process spawns, and leaves check ordering/short-circuit semantics homeless.
- **SARIF as the verdict format.** SARIF is file/line-centric and tool-oriented; we would still need the cell-implication layer on top, and consumers of SARIF (IDEs, code scanners) are not our consumers. A SARIF emitter can be derived from the v1 verdict later if wanted.
- **LLM judge maps diagnostics to cells.** Nondeterministic, costs tokens, unverifiable, and unnecessary — the mapping is mechanical once paths are declared.

## Test coverage

- A fixture audit emitting a v1 verdict with a cell-named failure causes re-dispatch of exactly that cell (graph + sandbox surfaces).
- A verdict failure implicating only files maps to the owning cell via declared `output_path`s.
- A `legacy-text` audit exiting 1 falls back to text extraction and the session record carries `verdict-noncompliant`; a `v1` audit exiting 1 without a parseable verdict is treated as `unrunnable`.
- The persisted SessionRecord carries per-check quads (name, status, implicated cells) queryable after the run.
