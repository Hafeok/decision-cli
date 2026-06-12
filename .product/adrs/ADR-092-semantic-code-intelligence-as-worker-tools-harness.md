---
id: ADR-092
title: 'Semantic code intelligence as worker tools: harness-owned LSP service, read tools first, symbol-level writes as a second slice'
status: proposed
features: []
supersedes: []
superseded-by: []
domains:
- api
- workers
scope: domain
---

**Status:** Proposed

## Context

The worker's entire view of code is textual. The tool registry (`workers/code-writer/src/code_writer/agent/tools.py`) offers `read_file` (line ranges), `write_file` (whole-file or `old_string` replacement), and the build/lint/test runners. The model has no way to ask *what symbols does this file declare*, *who calls this function*, or *did my edit type-check* — short of reading whole files and running a full `run_build`. Three witnessed costs:

- **Token spend on raw reads.** A worker orienting itself reads entire files when it needs an outline; understanding a call site means reading every plausible caller. This is the same money the FT-163 framing cap and the [ADR-090](ADR-090) budgets exist to bound.
- **Late, coarse verification.** The cheapest check after an edit is `run_build` over the workspace; cluster cells get no per-edit signal at all and rely on the post-hoc FT-172 compile probe — a whole-cluster audit round, with audit-repair dispatches ([FT-171](FT-171)) as the correction loop.
- **Fragile text edits.** `write_file` with `old_string` is anchored to exact text; renames and signature changes are multi-file find-and-replace performed by the model by hand.

The pipeline factory's code-intelligence layer is the proven counter-design (`docs/code-writer-mcp-server-spec.md`, its ADR-013/ADR-025): a **read server** (`find_definition`, `find_references`, `get_document_outline`, `search_symbols`, `get_symbol_body`, `get_diagnostics`) and a **write server** (`replace_symbol_body`, `insert_after/before_symbol`, workspace-wide `rename_symbol`, `apply_code_action`, `format_document`), both backed by one Roslyn LSP, with a two-tier symbol cache keyed by content hash (100–500 ms cold → <10 ms warm) and structured symbol-not-found errors that list the available symbols so the model self-corrects in one turn. The read/write split is structural security: the reader registers no write tools, so a read-only step cannot be coerced into writing.

Three structural mismatches stop a straight port:

1. **Stateless workers vs. a persistent index.** The factory amortises LSP startup inside a long-lived plant process. Our workers are stateless subprocesses ([ADR-008](ADR-008)); rust-analyzer cold-indexing this workspace per dispatch would cost more than it saves.
2. **Cluster sandboxes are not workspaces.** Cells emit fragments into `.dec/cluster/<ft>/` — not a compilable crate tree. [FT-172](FT-172)'s compile probe already solves this with a worktree-of-HEAD + overlay graft; LSP over cells needs the same construction.
3. **Language server fidelity.** Roslyn's symbol-level writes are best-in-class; rust-analyzer is strong on reads and diagnostics, weaker on write-shaped requests. Betting the first slice on the strong half de-risks the adoption.

Relation to [ADR-091](ADR-091) (SPMC): complementary, not overlapping. SPMC *pushes* a minimal, deterministic bundle at dispatch time (e.g. FT-177's `distill_rust_public_surface`). Semantic read tools are the *pull* side — the model retrieves exactly the symbol body or reference set its current step needs, on demand, with the compiler as the source of truth instead of a text heuristic. Pull-based retrieval is what makes aggressively minimal bundles safe: distilling an interface out of the bundle is acceptable when the worker can still query it if genuinely needed.

## Decision

**Add a semantic code-intelligence tool family to the worker, served by a harness-owned, run-scoped LSP service. Read tools land first; symbol-level write tools follow as a second slice once the read layer has proven itself. Tool grants flow through the role catalog like every other tool.**

1. **Run-scoped service, harness-owned.** The `dec` process spawns the language server (rust-analyzer in slice one) lazily on the first semantic tool call of a run, serves a thin local HTTP endpoint for the run's duration, and tears it down with the run. Workers receive the endpoint via a `code_intel_url` field on `DispatchPayload`; their semantic tools are thin clients. The worker stays stateless — the service holds a *workspace index*, not dispatch state, the same way the filesystem holds files.
2. **On-disk symbol cache keyed by content hash.** Symbol results are cached `(relative_path, content_hash)` à la the factory's raw tier, surviving across runs; a content change invalidates per file. Cold-start cost is paid once per workspace generation, not per dispatch.
3. **Read tool family (slice one).** `get_document_outline`, `get_symbol_body`, `find_definition`, `find_references`, `search_symbols`, `get_diagnostics`. Symbol-not-found errors are structured and list the symbols present in the file (the factory's one-turn self-correction). `get_diagnostics` gives per-edit compiler truth without a `run_build`.
4. **Write tool family (slice two, separate feature).** `replace_symbol_body`, `insert_after_symbol`, `insert_before_symbol`, `rename_symbol`, `format_document` — gated on the read slice's witnessed value and rust-analyzer's write-path fidelity at that time. Until then, `write_file` remains the only mutation path.
5. **Grants via the role catalog, split read from write.** The tools enter the worker registry and the role catalog as ordinary `dec:roleTool` entries ([ADR-070](ADR-070)), narrowable per cell ([ADR-088](ADR-088)). Read tools seed to implementer *and* verifier roles; write tools, when they land, seed to the implementer only. The structural read/write separation the factory achieves with two servers, we achieve with the catalog — which is already the enforcement point.
6. **Real workspaces first.** Slice one serves broad dispatches and worktree dispatches ([FT-115](FT-115)). Cluster-cell support requires the FT-172 worktree-overlay graft and arrives with slice two; until then cells keep their SPMC-distilled bundles ([FT-177](FT-177)) as the interface view.
7. **Fail-open absence.** A worker granted semantic tools but receiving no `code_intel_url` (legacy harness, service failed to start) drops the tools from the exposed registry and proceeds with the textual tools — semantic intelligence is an accelerator, not a dispatch precondition. The omission is recorded in worker telemetry.

## Rationale

- **Pull completes SPMC.** ADR-091 made bundles minimal by construction; the missing half is letting the model retrieve precisely what was distilled away when a step genuinely needs it. Outline-instead-of-file and symbol-body-on-demand attack the same token line items as the framing cap and budgets.
- **Per-edit diagnostics shorten the correction loop.** Today the first signal that an edit broke the build is a full `run_build` (broad path) or a failed audit round costing re-dispatches (cluster path). `get_diagnostics` moves that signal inside the same worker turn.
- **Run-scoped service fits the architecture we have.** It needs no resident daemon (none exists in slice 1), keeps the worker contract bundle-in/artifact-out, and the content-hash cache recovers the persistence benefit a daemon would give.
- **Read-first sequencing matches the risk.** The read tools are rust-analyzer's strong suite and carry no mutation risk; the write tools are both the harder LSP territory and the security-sensitive half. Proving value on the cheap, safe half first is the same graduated posture as ADR-085.

## Rejected alternatives

- **Per-dispatch LSP spawn inside the worker.** Pays rust-analyzer cold-start (tens of seconds on this workspace) on every dispatch; for short cell dispatches the spawn would dominate wall-clock. Also couples worker images to language-server binaries.
- **Resident system-wide LSP daemon.** Amortises best, but slice 1 has no daemon lifecycle (start/stop/health/upgrade ownership is unsettled), and a cross-run daemon holding workspace state invites staleness bugs the run-scoped service structurally avoids. Reconsider when a `dec` daemon exists for other reasons.
- **Harness-side pre-computation only (no interactive tools).** Pure push: embed outlines/diagnostics in the bundle and skip the service. Cheaper, but it re-creates the over-feeding problem SPMC just removed — the harness must guess what the worker will need. FT-177's distillation already covers the push side; the gap is precisely the on-demand half.
- **MCP servers like the factory (separate processes per concern).** Our workers are in-process agentic loops ([ADR-069](ADR-069)), not `claude -p` with `.mcp.json`; bolting an MCP client into the worker adds a protocol layer with no consumer besides ourselves. Thin HTTP against a harness service keeps the surface minimal. The factory's *split* survives via catalog grants; its *transport* does not need to.
- **Skip LSP, extend deterministic distillation.** `distill_rust_public_surface` is a text heuristic; it cannot answer references, definitions, or diagnostics, and growing it toward those answers re-implements a compiler badly.

## Test coverage

- A worker granted read tools resolves `get_document_outline`/`get_symbol_body` against a fixture crate through the run-scoped service; results name the fixture's real symbols.
- A repeated query on unchanged content is served from the content-hash cache without an LSP round-trip; a content change invalidates exactly that file's entry.
- Role-catalog seeds grant read tools to implementer and verifier; write tools absent everywhere in slice one (graph-level assertion).
- `get_diagnostics` reports a deliberately broken fixture edit within the worker session, with no `run_build` invocation in the telemetry.
- Absent `code_intel_url`, granted semantic tools are dropped from the exposed registry, the dispatch proceeds textually, and telemetry records the omission.
