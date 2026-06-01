---
id: TC-273
title: 'tool_safety: write_file with path traversal returns workspace violation'
type: scenario
status: unimplemented
validates:
  features:
  - FT-124
  adrs:
  - ADR-071
phase: 4
observes:
- exit-code
- disk-state
runner: pytest
runner-args: workers/_shared/tests/test_tool_safety.py::test_safe_join_containment
runner-timeout: 30
---

## Description

[ADR-071](ADR-071) requires `safe_join` to refuse any path that escapes the workspace, including via `..`, absolute paths, post-normalisation traversal, and symlinks. This TC pins each escape vector.

## Acceptance Criteria

Pytest test at `workers/_shared/tests/test_tool_safety.py::test_safe_join_containment`.

Setup: create a temporary directory `ws = tmp_path / "workspace"` and `ws.mkdir()`. Assert each case below:

**Valid paths (must return resolved Path inside workspace):**

- `safe_join(ws, "foo.txt")` returns `ws / "foo.txt"`.
- `safe_join(ws, "src/sub/foo.rs")` returns `ws / "src/sub/foo.rs"`.
- `safe_join(ws, ".")` returns `ws`.

**Traversal escapes (must raise WorkspaceViolation):**

- `safe_join(ws, "../escape.txt")`.
- `safe_join(ws, "src/../../../etc/passwd")`.
- `safe_join(ws, "/etc/passwd")` (absolute path outside).
- `safe_join(ws, "/")` (root).

**Symlink containment:**

- Create `(ws / "outside-link").symlink_to("/etc/passwd")`. `safe_join(ws, "outside-link")` raises `WorkspaceViolation`.
- Create `(ws / "inside-link").symlink_to(ws / "real.txt")` after `(ws / "real.txt").touch()`. `safe_join(ws, "inside-link")` succeeds and returns a path under `ws`.

**Disk-state invariant:**

After all assertions, no file outside `ws` has been created or modified. The test asserts `(tmp_path / "escape.txt").exists()` is False — `safe_join` is a pure resolution function; it never writes.
