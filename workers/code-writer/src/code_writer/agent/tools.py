"""Tool registry and dispatchers for the agent loop (FT-123 / ADR-071).

Five tool primitives per the spec:
- read_file
- write_file
- run_build
- run_lint
- run_tests

All tools enforce workspace containment and secrets blocking via
_shared.tool_safety per ADR-071.
"""

from __future__ import annotations

import os
import subprocess
from pathlib import Path
from typing import Any, Callable

try:
    from _shared.tool_safety import (
        MAX_TOOL_OUTPUT_BYTES,
        WorkspaceViolation,
        is_write_blocked,
        safe_join,
        tool_result_error,
    )
except ImportError:
    # Fallback for development/testing when _shared isn't installed
    from pathlib import Path

    class WorkspaceViolation(Exception):
        pass

    MAX_TOOL_OUTPUT_BYTES = 256 * 1024

    def safe_join(workspace: Path, requested: str) -> Path:
        workspace = workspace.resolve()
        if os.path.isabs(requested):
            resolved = Path(requested).resolve()
        else:
            resolved = (workspace / requested).resolve()
        try:
            resolved.relative_to(workspace)
        except ValueError:
            raise WorkspaceViolation(
                f"path {requested!r} resolves to {resolved} which escapes workspace {workspace}"
            )
        return resolved

    def is_write_blocked(path: Path) -> bool:
        from fnmatch import fnmatch

        blocked = {"*.env", "*.pem", "*.key", "*.pfx", "*.p12", "*.crt", "secrets.json", "secrets.yaml", "secrets.yml"}
        basename = path.name
        return any(fnmatch(basename, p) for p in blocked)

    def tool_result_error(message: str, **extra: Any) -> dict[str, Any]:
        result: dict[str, Any] = {"type": "tool_result", "is_error": True, "content": message}
        result.update(extra)
        return result


from ..models import FileWrite

# Tool schemas in OpenAI function-tools format (LiteLLM translates per-provider)
TOOL_SCHEMAS: dict[str, dict[str, Any]] = {
    "read_file": {
        "type": "function",
        "function": {
            "name": "read_file",
            "description": "Read a file in the workspace. Returns content with 1-indexed line numbers.",
            "parameters": {
                "type": "object",
                "properties": {
                    "path": {"type": "string", "description": "Workspace-relative path."},
                    "start_line": {"type": "integer", "minimum": 1},
                    "end_line": {"type": "integer", "minimum": 1},
                },
                "required": ["path"],
                "additionalProperties": False,
            },
        },
    },
    "write_file": {
        "type": "function",
        "function": {
            "name": "write_file",
            "description": "Write a file, or replace an exact string. Use old_string=null for full-file overwrite.",
            "parameters": {
                "type": "object",
                "properties": {
                    "path": {"type": "string"},
                    "content": {"type": "string"},
                    "old_string": {"type": ["string", "null"]},
                },
                "required": ["path", "content"],
                "additionalProperties": False,
            },
        },
    },
    "run_build": {
        "type": "function",
        "function": {
            "name": "run_build",
            "description": "Run the project build. Returns combined stdout/stderr + exit code.",
            "parameters": {
                "type": "object",
                "properties": {
                    "target": {
                        "type": "string",
                        "enum": ["rust", "python", "all"],
                        "default": "all",
                    }
                },
                "additionalProperties": False,
            },
        },
    },
    "run_lint": {
        "type": "function",
        "function": {
            "name": "run_lint",
            "description": "Run the project linter. Returns diagnostics.",
            "parameters": {
                "type": "object",
                "properties": {
                    "scope": {"type": "string", "enum": ["rust", "python", "all"], "default": "all"}
                },
                "additionalProperties": False,
            },
        },
    },
    "run_tests": {
        "type": "function",
        "function": {
            "name": "run_tests",
            "description": "Run the project test suite. Optional filter.",
            "parameters": {
                "type": "object",
                "properties": {
                    "scope": {"type": "string", "enum": ["rust", "python", "all"], "default": "all"},
                    "filter": {
                        "type": "string",
                        "description": "Test-name filter (cargo test / pytest -k).",
                    },
                },
                "additionalProperties": False,
            },
        },
    },
}


ToolDispatcher = Callable[[Path, dict[str, Any]], tuple[str, bool, FileWrite | None]]


def dispatch_read_file(workspace: Path, args: dict[str, Any]) -> tuple[str, bool, FileWrite | None]:
    """Read file tool dispatcher."""
    path_str = args.get("path", "")
    try:
        resolved = safe_join(workspace, path_str)
    except WorkspaceViolation as exc:
        return str(exc), True, None

    if not resolved.exists():
        return f"file not found: {path_str}", True, None

    try:
        content = resolved.read_text(encoding="utf-8")
    except Exception as exc:
        return f"read error: {exc}", True, None

    # Apply line range if requested
    start_line = args.get("start_line")
    end_line = args.get("end_line")
    if start_line is not None or end_line is not None:
        lines = content.splitlines()
        start_idx = (start_line - 1) if start_line else 0
        end_idx = end_line if end_line else len(lines)
        lines = lines[start_idx:end_idx]
        # Add line numbers
        numbered = "\n".join(f"{i + start_idx + 1}\t{line}" for i, line in enumerate(lines))
        return numbered, False, None

    # Add line numbers to full content
    numbered = "\n".join(f"{i + 1}\t{line}" for i, line in enumerate(content.splitlines()))
    return numbered, False, None


def dispatch_write_file(
    workspace: Path, args: dict[str, Any]
) -> tuple[str, bool, FileWrite | None]:
    """Write file tool dispatcher."""
    path_str = args.get("path", "")
    content = args.get("content", "")
    old_string = args.get("old_string")

    try:
        resolved = safe_join(workspace, path_str)
    except WorkspaceViolation as exc:
        return str(exc), True, None

    # Secrets blocking on writes
    if is_write_blocked(resolved):
        return f"write to {path_str} blocked: secrets pattern", True, None

    # Ensure parent directory exists
    resolved.parent.mkdir(parents=True, exist_ok=True)

    if old_string is None:
        # Full-file overwrite
        try:
            resolved.write_text(content, encoding="utf-8")
        except Exception as exc:
            return f"write error: {exc}", True, None
        file_write = FileWrite(path=path_str, summary="full-file write", bytes_written=len(content))
        return f"wrote {len(content)} bytes to {path_str}", False, file_write
    else:
        # Replace exact string
        if not resolved.exists():
            return f"file not found for string replacement: {path_str}", True, None
        try:
            existing = resolved.read_text(encoding="utf-8")
        except Exception as exc:
            return f"read error: {exc}", True, None
        if old_string not in existing:
            return f"old_string not found in {path_str}", True, None
        new_content = existing.replace(old_string, content, 1)
        try:
            resolved.write_text(new_content, encoding="utf-8")
        except Exception as exc:
            return f"write error: {exc}", True, None
        file_write = FileWrite(
            path=path_str, summary="string replacement", bytes_written=len(new_content)
        )
        return f"replaced string in {path_str}", False, file_write


def dispatch_run_build(
    workspace: Path, args: dict[str, Any]
) -> tuple[str, bool, FileWrite | None]:
    """Run build tool dispatcher."""
    target = args.get("target", "all")
    # Simple implementation: run cargo build for rust, or appropriate commands
    cmd = []
    if target == "rust" or target == "all":
        cmd = ["cargo", "build", "--workspace"]
    elif target == "python":
        cmd = ["python", "-m", "build"]

    if not cmd:
        return f"unknown build target: {target}", True, None

    try:
        result = subprocess.run(
            cmd,
            cwd=str(workspace),
            capture_output=True,
            text=True,
            timeout=120,
        )
    except subprocess.TimeoutExpired:
        return "build timed out after 120s", True, None
    except Exception as exc:
        return f"build error: {exc}", True, None

    output = result.stdout + result.stderr
    if len(output) > MAX_TOOL_OUTPUT_BYTES:
        output = output[-MAX_TOOL_OUTPUT_BYTES:]
    return f"exit_code={result.returncode}\n{output}", result.returncode != 0, None


def dispatch_run_lint(
    workspace: Path, args: dict[str, Any]
) -> tuple[str, bool, FileWrite | None]:
    """Run lint tool dispatcher."""
    scope = args.get("scope", "all")
    cmd = []
    if scope == "rust" or scope == "all":
        cmd = ["cargo", "clippy", "--workspace"]
    elif scope == "python":
        cmd = ["ruff", "check", "."]

    if not cmd:
        return f"unknown lint scope: {scope}", True, None

    try:
        result = subprocess.run(
            cmd,
            cwd=str(workspace),
            capture_output=True,
            text=True,
            timeout=120,
        )
    except subprocess.TimeoutExpired:
        return "lint timed out after 120s", True, None
    except Exception as exc:
        return f"lint error: {exc}", True, None

    output = result.stdout + result.stderr
    if len(output) > MAX_TOOL_OUTPUT_BYTES:
        output = output[-MAX_TOOL_OUTPUT_BYTES:]
    return f"exit_code={result.returncode}\n{output}", result.returncode != 0, None


def dispatch_run_tests(
    workspace: Path, args: dict[str, Any]
) -> tuple[str, bool, FileWrite | None]:
    """Run tests tool dispatcher."""
    scope = args.get("scope", "all")
    filter_arg = args.get("filter")

    cmd = []
    if scope == "rust" or scope == "all":
        cmd = ["cargo", "test", "--workspace"]
        if filter_arg:
            cmd.append(filter_arg)
    elif scope == "python":
        cmd = ["pytest"]
        if filter_arg:
            cmd.extend(["-k", filter_arg])

    if not cmd:
        return f"unknown test scope: {scope}", True, None

    try:
        result = subprocess.run(
            cmd,
            cwd=str(workspace),
            capture_output=True,
            text=True,
            timeout=120,
        )
    except subprocess.TimeoutExpired:
        return "tests timed out after 120s", True, None
    except Exception as exc:
        return f"test error: {exc}", True, None

    output = result.stdout + result.stderr
    if len(output) > MAX_TOOL_OUTPUT_BYTES:
        output = output[-MAX_TOOL_OUTPUT_BYTES:]
    return f"exit_code={result.returncode}\n{output}", result.returncode != 0, None


# Tool registry mapping name to dispatcher function
TOOL_REGISTRY: dict[str, ToolDispatcher] = {
    "read_file": dispatch_read_file,
    "write_file": dispatch_write_file,
    "run_build": dispatch_run_build,
    "run_lint": dispatch_run_lint,
    "run_tests": dispatch_run_tests,
}
