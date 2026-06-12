---
id: ADR-091
title: Hallucination is a context defect — SPMC context surgery over model escalation
status: accepted
features: []
supersedes: []
superseded-by: []
domains:
- workers
scope: cross-cutting
content-hash: sha256:4f86c3a80de333cde241c5c601f3613bcd4974a09c79261ed99c5e81bff97968
---

## Context

Nine witnessed cluster runs against FT-148 (June 2026) isolated every remaining dispatch failure to the two cells whose bundles were over-fed: full feature-spec prose plus five complete upstream artifact bodies. The failure shapes were classic hallucination signatures — inventing vocab files for types mentioned only in spec prose the cell never needed, spinning to the 40-turn cap, ending "ok" without producing the artifact. The infrastructure (placement, retries, timeouts, audits — FT-170/171/172) was eliminated as a cause run by run; the correlation that remained was bundle size and bundle *relevance*.

Two responses were on the table: escalate the failing cells to a stronger model, or apply the system's own decomposition principle to the bundles themselves. Escalation works sometimes — and that is exactly its danger: it hides the context defect, raises cost per dispatch, and leaves every future cell with the same failure mode at a higher price point. The capability funnel (ADR-037) was designed to reserve expensive models for genuine reasoning depth, not to compensate for careless context assembly.

## Decision

**When an LLM-backed step hallucinates, drifts, or fails to converge, treat it as a context defect. The first and default response is context surgery — SPMC (Single-Purpose, Minimal Context) — never model escalation.**

1. **Audit the bundle before blaming the model.** Diff what the step received against what its single purpose requires. Hallucinated content traces to bundle ingredients the step never needed.
2. **SPMC composition rules.** A dispatched unit receives: the spec *section* relevant to it (or none — downstream units' truth is the upstream artifacts), upstream *interfaces* rather than implementation bodies (deterministic distillation, no LLM in the path), and nothing else. A unit that "needs everything" is too big: split it, recursively applying the cell pattern.
3. **Context contracts are explicit and graph-resident.** Framing modes, distillation flags, and upstream selections are declared on the dispatched unit (`CellDecl.framing`, `CellDecl.distill_upstream` — FT-177), not buried in prompt-builder code, so the contract is auditable and amendable per unit.
4. **Escalation remains available only for reasoning depth** — a unit whose minimal context is already right but whose task genuinely exceeds the default capability. Escalating with a bloated bundle is forbidden; fix the bundle first.

## Rejected alternatives

- **Per-cell capability escalation as the first response.** Works, hides the defect, compounds cost. Rejected; remains a later lever for genuinely deep cells after SPMC.
- **Bigger context windows / higher turn caps.** More room to drown in; the FT-163/FT-164 history shows caps are safety nets, not fixes.
- **Keeping the rule in CLAUDE.md only.** Per ADR-014, rules that live outside the graph never reach the implementer's bundle. CLAUDE.md carries the human-facing statement; this ADR is the graph-resident, bundle-carried form.

## Consequences

- Every future feature bundle surfaces this rule (cross-cutting), including the bundles assembled for LLM workers — the rule reaches the things it governs.
- Debugging dispatch failures starts with a bundle diff, which FT-171's forensic dumps (`.cell-debug/`) make cheap.
- TaskType authoring must declare context contracts per cell; "give it everything" is a smell reviewers reject.
- Witnessed enforcement: FT-177 (per-cell framing, public-surface distillation, split test cells) is the first application; its TCs pin the invariant that Minimal-framed cells carry no spec prose.

## Status

Accepted. Worked example: FT-177; companion human-facing statement in CLAUDE.md §"Hallucination is a context defect".