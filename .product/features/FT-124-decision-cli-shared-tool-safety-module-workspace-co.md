---
id: FT-124
title: 'decision-cli: shared tool_safety module enforces workspace containment and secrets blocking'
phase: 4
status: planned
depends-on: []
adrs:
- ADR-071
- ADR-070
tests:
- TC-272
- TC-273
- TC-274
domains:
- security
domains-acknowledged:
  security: "New Python module `workers/_shared/src/_shared/tool_safety.py` consumed by every in-process tool primitive across all workers. Enforces workspace containment (path traversal refused, symlinks resolved + checked) and secrets blocking on writes (`.env`, `.pem`, `.key`, `.crt`, `secrets.{json,yaml,yml}`, …). Defense in depth at the tool boundary."
---

## Description

[ADR-071](ADR-071) requires every in-process tool primitive in every `dec` worker to route filesystem reads, writes, and subprocess invocations through a shared safety module. This feature delivers that module: `workers/_shared/src/_shared/tool_safety.py` with `safe_join`, `is_write_blocked`, and `tool_result_error`. It lands **before** FT-123 deliberately — the safety primitives have their own unit tests independent of any live LLM loop, so the property is locked down by the time FT-123 wires it into the agentic loop.

The module is a small, focused boundary: a handful of pure-ish functions whose contracts are tested in isolation. After this feature, the implementer for FT-123 can lean on `from _shared.tool_safety import safe_join, is_write_blocked, tool_result_error` without having to invent the safety semantics from scratch.

## Functional Specification

### Inputs

Module-level constants:

- `WRITE_BLOCKED_PATTERNS: tuple[re.Pattern, ...]` — compiled regex patterns matching `*.env`, `*.pem`, `*.key`, `*.pfx`, `*.p12`, `*.crt`, `secrets.{json,yaml,yml}`, `appsettings.production*` (case-insensitive). Exported for tests.
- `MAX_TOOL_OUTPUT_BYTES: int = 262_144` (256 KiB). Run-tool output cap.

Function signatures:

```python
def safe_join(workspace: Path, requested: str | Path) -> Path:
    """Resolve `requested` against `workspace`, returning the absolute Path.

    Normalises `..`, symlinks, and redundant slashes. Returns the resolved
    Path if and only if it is a descendant of (or equal to) `workspace`.
    Raises `WorkspaceViolation` otherwise.
    """

def is_write_blocked(workspace_relative_path: str | Path) -> bool:
    """True if the basename or full path matches any WRITE_BLOCKED_PATTERNS."""

def tool_result_error(tool_use_id: str, message: str) -> dict:
    """Return the LiteLLM/OpenAI-shaped tool-result error block:
        {"type": "tool_result", "tool_use_id": ..., "content": [{"type": "text", "text": message}], "is_error": True}
    Used by tool dispatchers to surface structured errors to the model.
    """

class WorkspaceViolation(Exception):
    """Raised by safe_join when the resolved path escapes workspace."""
```

### Outputs

- New file `workers/_shared/src/_shared/tool_safety.py`. Approximate LoC budget: 100, well under the 400-line cap from [ADR-013](ADR-013).
- The `_shared` package gains the symbols above on its `__init__.py` re-exports (mirrors the existing `_shared` packaging pattern used by `pipeline-worker-sdk` and its siblings).
- Unit tests at `workers/_shared/tests/test_tool_safety.py` (or per-worker tests under `workers/code-writer/tests/test_tool_safety.py` if `_shared` does not yet have its own test directory — implementation discovers).

### Behaviour

1. `safe_join(workspace, requested)`:
   - Coerce `workspace` and `requested` to `pathlib.Path`.
   - If `requested` is absolute, treat the absolute path as the candidate; otherwise resolve `(workspace / requested)`.
   - Call `.resolve(strict=False)` on both `workspace` and the candidate (resolves symlinks; tolerates non-existent leaf).
   - Assert `workspace.resolve()` is a parent of (or equal to) `candidate.resolve()` via `candidate.resolve().is_relative_to(workspace.resolve())` (Python 3.9+).
   - Return the resolved candidate on success. Raise `WorkspaceViolation(message=...)` with a structured message naming the offending path on failure.
2. `is_write_blocked(path)`:
   - Coerce to `pathlib.Path`. Match the *basename* and the *full path* (as string) against each compiled regex. Return True on first match.
   - Patterns are case-insensitive (compiled with `re.IGNORECASE`).
3. `tool_result_error(tool_use_id, message)`:
   - Return a dict literal exactly in the LiteLLM/OpenAI tool-result shape. The shape is documented inline so the format is checkable by reviewers.

### Acceptance criteria

- `safe_join(/tmp/ws, "src/foo.rs")` returns `/tmp/ws/src/foo.rs`.
- `safe_join(/tmp/ws, "../escape.txt")` raises `WorkspaceViolation`.
- `safe_join(/tmp/ws, "/etc/passwd")` raises `WorkspaceViolation` (absolute path outside workspace).
- `safe_join(/tmp/ws, "subdir/../../escape")` raises `WorkspaceViolation` (post-normalisation escape).
- Symlink containment: given a symlink at `/tmp/ws/link -> /etc/passwd`, `safe_join(/tmp/ws, "link")` raises `WorkspaceViolation`.
- `is_write_blocked(".env")` → True. `is_write_blocked("config/.env")` → True. `is_write_blocked("config/.env.example")` → False (`.env.example` does not match `*.env$`). `is_write_blocked("key.pem")` → True. `is_write_blocked("secrets.yaml")` → True. `is_write_blocked("secrets.txt")` → False.
- `tool_result_error("toolu_001", "blocked")` returns a dict with `"is_error": True` and `"tool_use_id": "toolu_001"`.
- All tests run via `pytest` from the repo root with `pytest workers/_shared/tests/test_tool_safety.py` (or wherever the test file lands).

## Out of scope

- Subprocess invocation helpers (`run_*` tools with `cwd=workspace` and timeouts). Owned by FT-123 (the tool implementations themselves), which import this module's `safe_join` and `is_write_blocked` plus apply their own subprocess containment per [ADR-071](ADR-071) §4-5.
- Static scan that validates every worker file imports `tool_safety`. Tracked as part of the [ADR-071](ADR-071) cross-cutting fitness gate; lives in `scripts/checks/tool-safety-imports.sh`, authored separately when the platform TC for that ADR lands.
- Sandboxing (Docker, firejail, nsjail). Out of scope per [ADR-071](ADR-071) alternatives considered.

## Exit Criteria (Test Coverage)

Per [ADR-013](ADR-013), behaviours above are asserted by TCs linked to this feature.
