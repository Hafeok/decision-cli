"""Workspace containment and secrets blocking for in-process worker tools (ADR-071).

Every in-process tool primitive MUST route filesystem reads, writes, and
subprocess invocations through this module to enforce:

1. **Workspace containment** — paths are resolved against the dispatch's
   workspace_path and rejected if they escape.
2. **Secrets blocking on writes** — writes to files matching blocked
   patterns (*.env, *.pem, secrets.json, etc.) are refused with a
   structured error.

Reads of secrets-pattern files are permitted (the model may need to read
.env.example); writes are unconditionally blocked.
"""

from __future__ import annotations

import os
from pathlib import Path
from typing import Any


class WorkspaceViolation(Exception):
    """Raised when a requested path escapes the workspace boundary."""

    pass


# Secrets patterns per ADR-071 — writes to these are blocked
WRITE_BLOCKED_PATTERNS = {
    "*.env",
    "*.pem",
    "*.key",
    "*.pfx",
    "*.p12",
    "*.crt",
    "secrets.json",
    "secrets.yaml",
    "secrets.yml",
}

# Maximum tool output size to prevent unbounded log growth
MAX_TOOL_OUTPUT_BYTES = 256 * 1024  # 256 KiB


def safe_join(workspace: Path, requested: str) -> Path:
    """Resolve a requested path against workspace with containment check.

    Args:
        workspace: The workspace root path.
        requested: User/model-provided path (absolute or relative).

    Returns:
        Resolved absolute path inside workspace.

    Raises:
        WorkspaceViolation: If the resolved path escapes workspace.
    """
    workspace = workspace.resolve()
    if os.path.isabs(requested):
        resolved = Path(requested).resolve()
    else:
        resolved = (workspace / requested).resolve()

    # Check if resolved path is inside workspace
    # Use os.path.commonpath to handle symlinks properly
    try:
        common = Path(os.path.commonpath([workspace, resolved]))
        if common != workspace and not str(resolved).startswith(str(workspace) + os.sep):
            raise WorkspaceViolation(
                f"path {requested!r} resolves to {resolved} which escapes workspace {workspace}"
            )
    except ValueError:
        # Different drives on Windows
        raise WorkspaceViolation(
            f"path {requested!r} resolves to {resolved} which escapes workspace {workspace}"
        )

    return resolved


def is_write_blocked(path: Path) -> bool:
    """Check if a path matches any write-blocked pattern.

    Args:
        path: The resolved path to check.

    Returns:
        True if the path basename or any component matches a blocked pattern.
    """
    from fnmatch import fnmatch

    basename = path.name
    for pattern in WRITE_BLOCKED_PATTERNS:
        if fnmatch(basename, pattern) or fnmatch(str(path), pattern):
            return True
    return False


def tool_result_error(message: str, **extra: Any) -> dict[str, Any]:
    """Build an LiteLLM/OpenAI-shaped tool_result error dict.

    Args:
        message: The error message to surface to the model.
        **extra: Additional fields to include in the result.

    Returns:
        A dict suitable for inclusion in messages as a tool result.
    """
    result: dict[str, Any] = {
        "type": "tool_result",
        "is_error": True,
        "content": message,
    }
    result.update(extra)
    return result
