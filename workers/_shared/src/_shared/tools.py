"""Canonical implementer tool schemas in OpenAI function-tools format (FT-060)."""

from __future__ import annotations

from typing import Any

#: The implementer's tool list per PRD §10.6 (FT-060). Declared once in
#: OpenAI function-tools format; translated to Anthropic ``tool_use`` shape
#: at call time via :func:`openai_tool_to_anthropic`.
IMPLEMENTER_TOOLS: list[dict[str, Any]] = [
    {
        "type": "function",
        "function": {
            "name": "read_file",
            "description": "Read the contents of a file from the workspace.",
            "parameters": {
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Absolute or workspace-relative path to read.",
                    },
                },
                "required": ["path"],
                "additionalProperties": False,
            },
        },
    },
    {
        "type": "function",
        "function": {
            "name": "edit_file",
            "description": "Edit a file by replacing exact text. Creates the file if missing.",
            "parameters": {
                "type": "object",
                "properties": {
                    "path": {"type": "string"},
                    "old_string": {"type": "string"},
                    "new_string": {"type": "string"},
                },
                "required": ["path", "new_string"],
                "additionalProperties": False,
            },
        },
    },
    {
        "type": "function",
        "function": {
            "name": "run_bash",
            "description": "Execute a bash command in the workspace and return stdout/stderr.",
            "parameters": {
                "type": "object",
                "properties": {
                    "command": {"type": "string"},
                    "timeout_seconds": {"type": "integer", "minimum": 1},
                },
                "required": ["command"],
                "additionalProperties": False,
            },
        },
    },
    {
        "type": "function",
        "function": {
            "name": "record_emergent_judgment",
            "description": (
                "Record an in-scope judgment the worker made unilaterally "
                "(per ADR-027 authority bounds)."
            ),
            "parameters": {
                "type": "object",
                "properties": {
                    "category": {"type": "string"},
                    "rationale": {"type": "string"},
                },
                "required": ["category", "rationale"],
                "additionalProperties": False,
            },
        },
    },
    {
        "type": "function",
        "function": {
            "name": "file_feedback",
            "description": (
                "Emit a feedback artifact when the worker hits a "
                "mustEscalate category (ADR-022 / ADR-027)."
            ),
            "parameters": {
                "type": "object",
                "properties": {
                    "feedback_class": {
                        "type": "string",
                        "enum": [
                            "gap",
                            "contradiction",
                            "unimplementable",
                            "scope-issue",
                            "defect",
                            "capability-request",
                        ],
                    },
                    "severity": {
                        "type": "string",
                        "enum": ["routine", "elevated", "critical"],
                    },
                    "target_role": {"type": "string"},
                    "rationale": {"type": "string"},
                    "evidence": {"type": "string"},
                },
                "required": ["feedback_class", "target_role", "rationale"],
                "additionalProperties": False,
            },
        },
    },
    {
        "type": "function",
        "function": {
            "name": "submit",
            "description": "Submit the final produced artifact to terminate the worker session.",
            "parameters": {
                "type": "object",
                "properties": {
                    "summary": {"type": "string"},
                },
                "required": ["summary"],
                "additionalProperties": False,
            },
        },
    },
]


def openai_tool_to_anthropic(tool: dict[str, Any]) -> dict[str, Any]:
    """Translate an OpenAI function-tools entry to Anthropic ``tool_use`` shape.

    The canonical declaration format across the framework is OpenAI
    function-tools (PRD §10.6); the Anthropic translation is mechanical
    and lossless — the function name, description, and JSON schema of
    its parameters become Anthropic's ``name``, ``description``, and
    ``input_schema`` respectively.
    """
    function = tool["function"]
    return {
        "name": function["name"],
        "description": function.get("description", ""),
        "input_schema": function["parameters"],
    }


def translate_tools_for_anthropic(tools: list[dict[str, Any]]) -> list[dict[str, Any]]:
    """Vectorised :func:`openai_tool_to_anthropic` for a list of tools."""
    return [openai_tool_to_anthropic(t) for t in tools]
