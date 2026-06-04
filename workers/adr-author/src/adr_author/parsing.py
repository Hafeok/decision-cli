"""Bare-ack defence plus JSON extraction helpers for the adr-author worker.

These are pure functions on text and dict payloads, factored out of
``worker.py`` to keep that module under ADR-013's per-file size limit.
The bare-ack defence (FT-130 §4B) is testable in isolation here.
"""

from __future__ import annotations

import json


# Brief §4B: an acknowledgement's reasoning must be ≥ 40 chars of
# substantive prose. Anything shorter is treated as bare and rejected
# at the worker boundary before stdout.
BARE_ACK_MIN_CHARS = 40


def check_bare_ack(payload: dict) -> str | None:
    """Detect a bare-ack payload BEFORE Pydantic validation.

    Returns a validation-error string when ``payload`` claims
    ``kind: "acknowledgement"`` and its ``reasoning`` field is missing,
    empty, whitespace-only, or shorter than :data:`BARE_ACK_MIN_CHARS`.
    Returns ``None`` otherwise.
    """
    if not isinstance(payload, dict):
        return None
    if payload.get("kind") != "acknowledgement":
        return None
    ack = payload.get("acknowledgement")
    if not isinstance(ack, dict):
        return "kind='acknowledgement' but `acknowledgement` payload is missing"
    reasoning = ack.get("reasoning", "")
    if not isinstance(reasoning, str):
        return "acknowledgement.reasoning must be a string"
    stripped = reasoning.strip()
    if not stripped:
        return (
            "bare-ack rejected at worker boundary: "
            "acknowledgement.reasoning is empty or whitespace-only "
            "(forbidden per FT-130 §4B)"
        )
    if len(stripped) < BARE_ACK_MIN_CHARS:
        return (
            "bare-ack rejected at worker boundary: "
            f"acknowledgement.reasoning stripped to {len(stripped)} chars "
            f"(< {BARE_ACK_MIN_CHARS}) per FT-130 §4B"
        )
    return None


def extract_json_object(text: str) -> dict | None:
    """Best-effort: find a top-level JSON object in the model output."""
    direct = _try_parse_json_dict(text)
    if direct is not None:
        return direct
    start = text.find("{")
    if start < 0:
        return None
    end = _find_balanced_brace_end(text, start)
    if end < 0:
        return None
    return _try_parse_json_dict(text[start : end + 1])


def _try_parse_json_dict(text: str) -> dict | None:
    """Parse ``text`` as JSON, returning the value only when it is a dict."""
    try:
        parsed = json.loads(text)
    except json.JSONDecodeError:
        return None
    return parsed if isinstance(parsed, dict) else None


def _find_balanced_brace_end(text: str, start: int) -> int:
    """Return index of the `}` closing the `{` at ``start``, or -1 if unbalanced."""
    depth = 0
    in_str = False
    escape = False
    for i in range(start, len(text)):
        ch = text[i]
        if in_str:
            in_str, escape = _advance_string_state(ch, escape)
            continue
        if ch == '"':
            in_str = True
            continue
        if ch == "{":
            depth += 1
        elif ch == "}":
            depth -= 1
            if depth == 0:
                return i
    return -1


def _advance_string_state(ch: str, escape: bool) -> tuple[bool, bool]:
    """Update (in_string, escape) flags for one character inside a JSON string."""
    if escape:
        return True, False
    if ch == "\\":
        return True, True
    if ch == '"':
        return False, False
    return True, False
