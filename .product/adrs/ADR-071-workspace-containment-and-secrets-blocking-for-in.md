---
id: ADR-071
title: Workspace containment and secrets blocking for in-process worker tools
status: accepted
features: []
supersedes: []
superseded-by: []
domains:
- workers
scope: domain
content-hash: sha256:200d444f220d9d9d90ce21f91bd348630d0775fe3df4e7faa21231b091f33e7f
source-files:
- workers/_shared/src/_shared/tool_safety.py
---

## Context

[ADR-069](ADR-069) moves the code-writer worker from a subprocess (`claude -p`) to an in-process LiteLLM-client agentic loop. [ADR-070](ADR-070) gives every role a declared tool surface. Together these decisions move the question of "what can the model do to the filesystem" inside our Python process for the first time.

Previously, the subprocess was a soft fence — `claude -p --dangerously-skip-permissions` ran with whatever the operating user could do. The blast radius was the user's filesystem, but the loop was opaque; any logging of refusals happened inside Claude Code. With the in-process loop, the worker owns the tool implementations, so it owns the safety policy. That is both a risk (we hold the gun) and an opportunity (we get to write the policy down).

Two concrete failure modes motivate this ADR:

1. **Path traversal.** A model that proposes `write_file(path="../../etc/passwd")` or `write_file(path="/home/user/.ssh/id_rsa")` would, without containment, write outside the workspace. The pipeline factory's MCP server already handles this (`/home/hafeok/projects/pipeline/mcp-servers/src/code-writer/index.ts` lines 31, 36-39): every path resolves under `WORKSPACE`, anything outside is rejected.
2. **Secrets exfiltration / corruption.** A model that proposes `write_file(path=".env", content="...")` could either overwrite credentials (corruption) or stage a file the operator commits by accident (exfiltration if pushed to a remote). The pipeline factory blocks a regex set (`.env`, `.pem`, `.key`, `.pfx`, `.p12`, `.crt`, `secrets.{json,yaml,yml}`, `appsettings.production*`) at the tool implementation. Block-list-by-pattern is imperfect but high-value-cheap.

Borrowing the pattern from the pipeline factory is intentional. The same pattern works inside `dec` because the threat shape is identical: an LLM proposes a tool call against a workspace; the worker validates and either executes or returns a structured error to the model. We do not borrow the JWT step-token / MCP-server-per-step deployment cost — that was a multi-process control-plane decision driven by the factory's distributed runtime.

The decision lives at ADR scope because every in-process tool that ever lands in `dec` (today: `read_file`, `write_file`, `run_build`, `run_lint`, `run_tests`; tomorrow: whatever the reviewer / refactorer / summariser roles need) inherits the same containment and blocking rules. Encoding this once at the worker boundary, rather than per-tool, is the difference between a coherent safety story and a kit of ad-hoc checks.

There is **no overlap with the harness or graph layer.** `dec` itself has bigger blast radius (it can write to `.dec/store/`, mutate the orchestration graph, etc.) — but `dec`'s actions are operator-initiated, not model-initiated. This ADR governs the *model-initiated* tool surface inside the worker, not the operator-initiated surface of the CLI.

## Decision

Every in-process tool primitive in a `dec` worker MUST route its filesystem reads, writes, and subprocess invocations through a shared safety module that enforces two invariants:

1. **Workspace containment.** Every path is resolved against the dispatch's `workspace_path` via a `_safe_join(workspace, requested)` helper that:
   - Normalises the requested path (resolves `..`, symlinks, redundant slashes).
   - Asserts the resolved path is a descendant of `workspace_path` (or equal to it).
   - Returns `Result[Path, WorkspaceViolation]`; tools translate `WorkspaceViolation` into a structured `tool_result` error block returned to the model — never an uncaught Python exception, never a silent path coercion.
2. **Secrets blocking on writes.** Before any write or create, the path's basename and full workspace-relative path are matched against `WRITE_BLOCKED_PATTERNS`:
   ```
   *.env             *.pem             *.key
   *.pfx             *.p12             *.crt
   secrets.json      secrets.yaml      secrets.yml
   ```
   A match returns `tool_result(is_error=true, content="write to <path> blocked: secrets pattern")` to the model. Reads of these files are permitted (the model may need to read `.env.example`, etc.); writes are unconditionally refused.

Concrete substance:

3. **Shared module.** The implementation lives in `workers/_shared/src/_shared/tool_safety.py` so the same code is importable from `code-writer`, `verifier`, `verify-graph-author`, and any future worker. The module exposes `safe_join(workspace, path) -> Path`, `is_write_blocked(path) -> bool`, and a `tool_result_error(message) -> dict` helper that returns the LiteLLM/OpenAI tool-result shape.
4. **Subprocess containment.** `run_build`, `run_lint`, `run_tests` execute with `cwd=workspace_path` (no opportunity for the model to chdir elsewhere); arguments are validated against an allowlist per tool (no shell metacharacters, no unbounded glob). Output is captured and bounded at `MAX_TOOL_OUTPUT_BYTES = 256 KiB` to prevent run-away log growth.
5. **Timeouts.** Each `run_*` invocation caps at `min(120s, payload.timeout_seconds // 4)`. The overall dispatch timeout still governs the loop; this is a per-tool guard against a wedged subprocess consuming the whole budget on one call.
6. **No symbolic-link escape.** `safe_join` resolves symlinks; a symlink inside the workspace pointing outside it is treated as outside (rejected).
7. **No exemptions per tool.** Every tool calls into the safety module; there is no `unsafe_write_file` variant. New tools that need write access write through `safe_join` first.

This is a **cross-cutting** ADR: the constraint applies to every in-process tool in every worker, today and in the future. Adding a new worker or a new tool primitive without using `_safe_join` and `is_write_blocked` is a TC failure under `product verify --platform`.

## Consequences

**Positive:**

- A single audit point. Reviewing whether a worker can be safely run against an arbitrary workspace reduces to "does it import `_shared.tool_safety`, and does every filesystem touch route through it?" — answerable by `grep`.
- The model receives structured `tool_result` errors instead of opaque failures, which materially improves the agentic loop's recovery semantics (the model can read the error and try a different path).
- Test surface is small and focused: a handful of unit tests against `_safe_join` and `is_write_blocked` cover the bulk of the invariant. The expensive end-to-end tests (live model, live workspace) get to skip the safety axis because the unit tests already cover it.
- Future workers inherit the safety story by importing the module. There is no per-worker "did you remember to validate paths" review burden.

**Negative / accepted costs:**

- Blocklist patterns are imperfect. A model determined to write secrets to a path the regex doesn't match (e.g. `auth/prod-key.json` — not blocked) can succeed. Mitigation: the block list is a defense-in-depth measure for honest mistakes, not a complete sandbox. Sandbox-level isolation (containers, syscall filters) is a separate concern handled at deployment time, not in this ADR.
- The `run_*` timeout-and-cwd model rules out tools that legitimately need to `cd` outside the workspace or run for minutes (e.g. a 10-minute full-repo Cargo build). Mitigation: bump the per-tool cap on a per-deployment basis if needed; long-running builds belong in a CI runner that the worker requests, not in the worker's own process.
- Symbolic-link resolution surprises operators who use symlinks to share files across workspaces. Mitigation: documented in the FT-124 spec; operators with this use case can vendor the file or override `tool_safety.WORKSPACE_RESOLUTION` (no override exists today — adding one is a feature_spec away).

**Boundary enforcement (cross-cutting fitness gate):**

The TC linked to this ADR fails the platform gate if any file under `workers/*/src/` performs `pathlib.Path(...)` joined with model-provided input without going through `safe_join`. The check is a static scan in `scripts/checks/tool-safety-imports.sh`; it is allowed to be conservative (false positives flagged for manual review) rather than permissive.

## Alternatives considered

- **Sandbox each tool call in a Docker / firejail / nsjail container.** Strongest isolation; highest operational cost. Rejected for slice 1 — the operational cost is unjustified for a worker invoked by a developer on their own machine. May revisit when `dec` runs unattended in CI or production deployments.
- **Per-tool path validation.** Each tool implements its own checks. Rejected: invites omissions and inconsistencies. The whole motivation for a cross-cutting ADR is that the rule is the same across tools, so the code should be the same too.
- **Allow-list-based write filter instead of block-list.** "Tools may only write files matching `src/**` `tests/**` `*.md`." Rejected as too restrictive for an implementer role that needs to author Cargo.toml updates, new TC scripts, etc. The block-list-on-secrets is the right shape for now; an allow-list could be revisited per-role via [ADR-070](ADR-070)'s sub-resource extension if needed.
- **Trust the model not to write secrets.** Rejected: explicit non-goal. Even if the model never proposed it, a future prompt-injection vector through bundle content could induce the proposal. Defense in depth at the tool boundary is the cheapest insurance against that class of attack.
- **Move containment into Rust at the harness layer.** The harness could check every artifact the worker returns and reject any path outside the workspace. Rejected as too late — by the time the artifact reaches the harness, the file has already been written. Containment has to live at the point of the system call.

## Status

Proposed. Once accepted, governs the FT-124 implementation (Python safety module + tests). [ADR-069](ADR-069) consumes the safety module from every tool primitive; [ADR-070](ADR-070) sets the role-level surface that determines *which* tools come under this safety regime per dispatch.
