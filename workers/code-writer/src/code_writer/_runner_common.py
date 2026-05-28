"""Shared helpers used by both the stub and real Claude runners."""

from __future__ import annotations

import os
import shutil
from pathlib import Path

STUB_ENV_VAR = "CODE_WRITER_STUB"


def _is_stub_mode() -> bool:
    return os.environ.get(STUB_ENV_VAR, "").strip() not in {"", "0", "false", "no"}


def _safe_join(workspace: Path, rel: str) -> Path:
    """Resolve ``rel`` against ``workspace`` and reject path traversal.

    ADR-008 invariant: files written must be confined to the workspace.
    Any attempt to escape via ``..`` or absolute paths is treated as
    ``workspace_violation``.
    """
    candidate = (workspace / rel).resolve()
    workspace_resolved = workspace.resolve()
    if not str(candidate).startswith(str(workspace_resolved)):
        raise ValueError(f"path {rel!r} escapes workspace {workspace!s}")
    return candidate


def _make_code_change_iri(dispatch_id: str) -> str:
    """Mint a deterministic CodeChange IRI tied to the dispatch."""
    if "://" in dispatch_id:
        head, _, tail = dispatch_id.rpartition("/")
        return f"{head}/code-change/{tail or 'unnamed'}"
    return f"urn:dec:code-change:{dispatch_id}"


def _claude_on_path() -> str | None:
    return shutil.which("claude")
