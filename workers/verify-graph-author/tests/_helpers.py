"""Synthetic bundle builders shared across the FT-048 TC suites."""

from __future__ import annotations

import json
from typing import Any

from verify_graph_author.bundle import VerifyGraphAuthorInput


BUNDLE_HASH = "deadbeef" * 8  # 64 chars, looks-like-sha256


SEED_VOCABULARY: list[dict[str, Any]] = [
    {
        "kind": "shell-command",
        "required_ops": ["shell"],
        "fields_schema": {
            "type": "object",
            "required": ["command"],
            "properties": {"command": {"type": "string"}},
        },
        "description": "Run a shell command and capture stdout/stderr/exit-code.",
    },
    {
        "kind": "sparql-assertion",
        "required_ops": ["sparql-readonly"],
        "fields_schema": {
            "type": "object",
            "required": ["query", "expected"],
            "properties": {
                "query": {"type": "string"},
                "expected": {"type": "object"},
            },
        },
        "description": "Assert a SPARQL ASK or count query against a store snapshot.",
    },
    {
        "kind": "file-assertion",
        "required_ops": ["file-read"],
        "fields_schema": {
            "type": "object",
            "required": ["path", "matcher"],
            "properties": {
                "path": {"type": "string"},
                "matcher": {"type": "object"},
            },
        },
        "description": "Assert a file exists and matches a content matcher.",
    },
    {
        "kind": "http-request",
        "required_ops": ["http-readonly", "http-mutating"],
        "fields_schema": {
            "type": "object",
            "required": ["method", "url"],
            "properties": {
                "method": {"type": "string"},
                "url": {"type": "string"},
                "body": {"type": "object"},
                "assert_status": {"type": "integer"},
            },
        },
        "description": "Issue an HTTP request and assert on the response.",
    },
    {
        "kind": "wait-for",
        "required_ops": [],
        "fields_schema": {
            "type": "object",
            "required": ["condition"],
            "properties": {"condition": {"type": "string"}, "timeout_seconds": {"type": "integer"}},
        },
        "description": "Block until a named condition holds.",
    },
    {
        "kind": "capture",
        "required_ops": [],
        "fields_schema": {
            "type": "object",
            "required": ["name", "from"],
            "properties": {"name": {"type": "string"}, "from": {"type": "string"}},
        },
        "description": "Capture an intermediate value for downstream steps.",
    },
]


def build_bundle_for_match() -> VerifyGraphAuthorInput:
    """TC-076 — synthetic bundle with a candidate covering all TCs."""
    return VerifyGraphAuthorInput.model_validate(
        {
            "feature_id": "FT-Q",
            "feature_spec": (
                "# FT-Q — example feature\n\n"
                "This is a synthetic feature for TC-076. It declares two TCs the "
                "verify-graph-author must propose evidence for, but a candidate "
                "graph already covers both."
            ),
            "relevant_tcs": [
                {"id": "T1", "title": "first TC", "body": "Body of T1."},
                {"id": "T2", "title": "second TC", "body": "Body of T2."},
            ],
            "target_environment": {
                "id": "ENV-1",
                "env_type": "ephemeral-tempdir",
                "safety_class": "read-write-local",
                "allowed_ops": ["shell", "file-read", "sparql-readonly"],
            },
            "candidate_graphs": [
                {
                    "id": "VG-K",
                    "verifies": "FT-Q",
                    "covers": ["T1", "T2"],
                    "step_summaries": [
                        {
                            "step_type": "shell-command",
                            "summary": "Run the FT-Q implementation under the tempdir env.",
                            "provides_evidence_for": ["T1"],
                        },
                        {
                            "step_type": "file-assertion",
                            "summary": "Assert the output file matches the FT-Q spec.",
                            "provides_evidence_for": ["T2"],
                        },
                    ],
                }
            ],
            "step_vocabulary": SEED_VOCABULARY,
            "bundle_hash": BUNDLE_HASH,
        }
    )


def build_bundle_for_gap_ops_mismatch() -> VerifyGraphAuthorInput:
    """TC-077 — env's allowed_ops cannot satisfy the TCs."""
    return VerifyGraphAuthorInput.model_validate(
        {
            "feature_id": "FT-R",
            "feature_spec": (
                "# FT-R — feature requiring mutating HTTP\n\n"
                "T1 demands asserting the side-effect of a POST request, but the "
                "target environment only permits http-readonly operations."
            ),
            "relevant_tcs": [
                {
                    "id": "T1",
                    "title": "POST side-effect",
                    "body": "Assert that a POST to /widgets persists the widget.",
                }
            ],
            "target_environment": {
                "id": "ENV-prod",
                "env_type": "http-endpoint",
                "safety_class": "production",
                "allowed_ops": ["http-readonly"],
                "endpoint": "https://api.example.com",
            },
            "candidate_graphs": [],
            "step_vocabulary": SEED_VOCABULARY,
            "bundle_hash": BUNDLE_HASH,
        }
    )


def build_bundle_for_protocol_check() -> VerifyGraphAuthorInput:
    """TC-078 — any valid bundle works; the test mocks a wrong-hash response."""
    return VerifyGraphAuthorInput.model_validate(
        {
            "feature_id": "FT-S",
            "feature_spec": (
                "# FT-S — bundle-hash echo check\n\n"
                "The model is mocked to return a proposal with a stale bundle_hash."
            ),
            "relevant_tcs": [
                {"id": "T1", "title": "trivial TC", "body": "anything"},
            ],
            "target_environment": {
                "id": "ENV-1",
                "env_type": "ephemeral-tempdir",
                "safety_class": "read-write-local",
                "allowed_ops": ["shell", "file-read"],
            },
            "candidate_graphs": [],
            "step_vocabulary": SEED_VOCABULARY,
            "bundle_hash": "abc123" + ("0" * 58),  # well-formed 64-char hash
        }
    )


def make_caller(*responses: dict | str):
    """Return a ModelCaller that yields each response in order, counting calls."""
    calls: list[tuple[str, str, str, int]] = []
    queue = list(responses)

    def caller(system: str, user: str, model_id: str, max_tokens: int):
        calls.append((system, user, model_id, max_tokens))
        if not queue:
            raise AssertionError("ModelCaller invoked more times than responses provided")
        head = queue.pop(0)
        raw = head if isinstance(head, str) else json.dumps(head)
        return raw, 100, 50

    caller.calls = calls  # type: ignore[attr-defined]
    return caller


def assert_no_anthropic_attempt(monkeypatch):
    """Replace the live anthropic caller with a sentinel that fails the test if invoked."""
    from verify_graph_author import worker as worker_mod

    def boom(*args, **kwargs):  # noqa: ARG001
        raise AssertionError(
            "live Anthropic call escaped the mock — test must inject a caller"
        )

    monkeypatch.setattr(worker_mod, "call_claude", boom)
