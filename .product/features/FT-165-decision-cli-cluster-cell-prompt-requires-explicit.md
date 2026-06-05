---
id: FT-165
title: 'decision-cli: cluster cell prompt requires explicit write_file invocation'
phase: 4
status: complete
depends-on:
- FT-139
- FT-164
adrs:
- ADR-080
tests:
- TC-403
- TC-404
- TC-405
- TC-406
domains:
- api
domains-acknowledged: {}
---

## Description

Third small fix to the cluster pattern after [FT-163](FT-163) (framing) and [FT-164](FT-164) (turn cap). Witnessed by FT-147 retries with both fixes in place: ~50% of cells in any given dispatch fail with "did not produce <file>" even though the worker has the full spec, 40 turns of headroom, and clear instructions to write a file. The model is occasionally returning `stop_reason: stop` without ever invoking the `write_file` tool — and one retry produced a stray `product.verify` file in the sandbox the worker invented on its own initiative.

The current prompt at `cluster_dispatch::build_cell_bundle` says:

```
## Your task

Emit the `<cell>` cell of the `<task-type>` task type. The artifact type is `<type>`.

Write a single file at the workspace-relative path `<path>` containing the cell's content. 
Do not produce any other files. Do not edit existing files. 
When you have written the file, end your turn.
```

Three failure modes the current prompt admits:

1. **"Write a single file"** is ambiguous — the model can interpret it as "produce file content in your assistant message", end its turn, and call that "writing a file". The harness then reads zero bytes from disk.
2. **No explicit tool requirement** — nothing tells the model that only a `write_file` tool call satisfies the dispatch.
3. **No anti-narrate guard** — the model defaults to summarizing what it's about to do, sometimes summarizing without doing.

The fix is prompt engineering. Tighten the instruction so a text-only response cannot satisfy the dispatch; a `write_file` tool call is **structurally required**.

## Functional Specification

### Inputs

- `cluster_dispatch::build_cell_bundle` — the per-cell bundle composer. The §"Your task" section at the tail is the per-cell instruction the worker reads.
- Witnessed FT-147 retries: 3 dispatches at €0.07 each, ~50% cells produced files, ~50% failed in variable positions (emitter once, parser twice, occasional stray `product.verify` file).
- The worker's tool surface from the role catalog — `read_file` and `write_file` already granted for cluster cells.

### Outputs

**Rewritten §"Your task" section** in `build_cell_bundle`:

```
## Your task

Emit the `<cell>` cell of the `<task-type>` task type. The artifact type is `<type>`.

### Required workflow

1. Call the `write_file` tool with:
   - `path`: `<target_filename>`
   - `content`: the **complete** file body — no placeholders, no TODO markers, no "rest of file unchanged".
2. The dispatch is INCOMPLETE until your `write_file` tool call returns success.
3. Do not paste the file content into your assistant message text — that is NOT writing the file. Only a `write_file` tool call counts.
4. Do not create any other files. The target path is the ONLY file you may write.
5. After `write_file` returns success, respond with a single line confirming success and end your turn.

### Failure modes to avoid

- Responding with file content in markdown but never calling `write_file` → the dispatch reads zero bytes and aborts.
- Calling `write_file` with partial content and a placeholder ("// ... rest unchanged") → the file fails the audit downstream.
- Creating helper / scratch files alongside the target → audit rejects.
- Narrating the plan before acting — call the tool first, narrate after.
```

### State

- **Modified on-disk:** `crates/decision-cli/src/features/drive/cluster_dispatch.rs` — `build_cell_bundle` only. The §"Your task" string body changes; surrounding logic untouched.
- **No new files, no config surface, no schema change, no orchestration-store mutation.**

### Behaviour

1. **Every LLM-backed cell dispatched through `emit_llm_cell`** receives the new instruction block. Mechanical cells unaffected (they never see a prompt).
2. **The instruction is the same shape across all cells** — same numbered list, same failure-modes list. No per-cell variation in v1.
3. **No cluster-execution semantics change** — same audit, fail-fast, FT-146 SessionRecord persistence, FT-163 framing, FT-164 turn cap.

### Invariants

- **The instruction names `write_file` by name.** No ambiguity about the tool to call.
- **The instruction explicitly forbids text-only responses.** Removes the "I'll just paste the content" failure mode.
- **The instruction caps file creation at the single target.** Removes the "let me also create a helper" failure mode (witnessed `product.verify` on FT-147 retry).
- **The instruction is task-shape-agnostic.** Works for Rust, Turtle, Python, Markdown — every artifact type the cluster currently emits.

### Error handling

- **Worker still fails to call `write_file`** despite the tightened prompt → existing "did not produce <file>" error, unchanged. FT-146 SessionRecord persists with `cellStatus=failed`. The prompt is a strong nudge, not a hard contract — the structural enforcement is at the harness boundary.
- **Worker calls `write_file` but with empty content** → file is written, audit reads it, audit script catches the empty body downstream. Outside this slice's scope.
- **Worker writes the target file plus a stray file** → audit catches it via the existing fixture checks (e.g., `cluster-audit-add-artifact-type.py` rejects on extra files).

### Boundaries

- **In scope.** Prompt rewrite + 4 TCs (the rewrite is tested by string-shape assertions; we don't dispatch live LLMs in TCs).
- **Out of scope.** Per-cell prompt variations. Few-shot exemplars (passing a prior successful cell output as an "example of what good looks like"). Tool-call validation on the worker side (refusing to return success without a `write_file` call) — that's a worker SDK change, separate slice. Adaptive prompts based on prior cell failures. Multi-language prompt files (i18n).

## Out of scope

- Per-cell prompt variations.
- Few-shot exemplars in the bundle.
- Worker-side enforcement of write_file call.
- Adaptive prompts.
- i18n.
