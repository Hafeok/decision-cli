"""Synthetic bundle builders shared across the FT-126 TC suites."""

from __future__ import annotations

import json
from typing import Any

from tc_author.bundle import TcAuthorInput


BUNDLE_HASH = "deadbeef" * 8  # 64 chars, looks-like-sha256

# Dispatcher-pinned identifiers used across the synthetic FT-126 bundles.
_FIXTURE_ENDPOINT = "anthropic"
_FIXTURE_MODEL_ID = "test-model-pinned-by-dispatcher"


def build_bundle_for_sufficient() -> TcAuthorInput:
    """TC-286 — synthetic bundle with existing TCs meeting target_count."""
    return TcAuthorInput.model_validate(
        {
            "endpoint": _FIXTURE_ENDPOINT,
            "model_id": _FIXTURE_MODEL_ID,
            "feature_id": "FT-stub",
            "feature_spec": (
                "# FT-stub — example feature\n\n"
                "This is a synthetic feature for TC-286. It already has three "
                "TCs that meet the target_count of 3."
            ),
            "existing_tcs": [
                {
                    "id": "TC-001",
                    "title": "Happy path test",
                    "status": "active",
                    "runner": "bash",
                    "runner_args": "tests/scripts/tc-001.sh",
                    "body": "## Purpose\nTests happy path.",
                },
                {
                    "id": "TC-002",
                    "title": "Edge case test",
                    "status": "active",
                    "runner": "cargo-test",
                    "runner_args": "test_edge",
                    "body": "## Purpose\nTests edge case.",
                },
                {
                    "id": "TC-003",
                    "title": "Integration test",
                    "status": "active",
                    "runner": "pytest",
                    "runner_args": "tests/test_integration.py::test_x",
                    "body": "## Purpose\nTests integration.",
                },
            ],
            "tc_schema": {
                "required_fields": ["title", "type", "runner", "runner-args"],
                "allowed_types": ["scenario", "exit-criteria", "invariant"],
                "allowed_runners": ["bash", "cargo-test", "pytest", "custom"],
            },
            "runner_vocabulary": [
                {
                    "kind": "bash",
                    "args_pattern": r".+\.sh$",
                    "timeout_default": "60s",
                },
                {
                    "kind": "cargo-test",
                    "args_pattern": r"^[a-zA-Z_][a-zA-Z0-9_]*$",
                    "timeout_default": "120s",
                },
                {
                    "kind": "pytest",
                    "args_pattern": r"^.+\.py::.+$",
                    "timeout_default": "60s",
                },
            ],
            "target_count": 3,
            "coverage_axes": [
                {"axis": "happy", "description": "Primary intended behaviour"},
                {"axis": "edge", "description": "Boundary conditions"},
                {"axis": "integration", "description": "Interaction with adjacent features"},
                {"axis": "state", "description": "Side effects"},
            ],
            "bundle_hash": BUNDLE_HASH,
        }
    )


def build_bundle_for_new() -> TcAuthorInput:
    """TC-287 — synthetic bundle with zero existing TCs, target_count=4."""
    return TcAuthorInput.model_validate(
        {
            "endpoint": _FIXTURE_ENDPOINT,
            "model_id": _FIXTURE_MODEL_ID,
            "feature_id": "FT-stub",
            "feature_spec": (
                "# FT-stub — example feature\n\n"
                "This is a synthetic feature for TC-287. It has no existing "
                "TCs and needs exactly 4 new ones."
            ),
            "existing_tcs": [],
            "tc_schema": {
                "required_fields": ["title", "type", "runner", "runner-args"],
                "allowed_types": ["scenario", "exit-criteria", "invariant"],
                "allowed_runners": ["bash", "cargo-test", "pytest", "custom"],
            },
            "runner_vocabulary": [
                {
                    "kind": "bash",
                    "args_pattern": r".+\.sh$",
                    "timeout_default": "60s",
                },
                {
                    "kind": "cargo-test",
                    "args_pattern": r"^[a-zA-Z_][a-zA-Z0-9_]*$",
                    "timeout_default": "120s",
                },
                {
                    "kind": "pytest",
                    "args_pattern": r"^.+\.py::.+$",
                    "timeout_default": "60s",
                },
            ],
            "target_count": 4,
            "coverage_axes": [
                {"axis": "happy", "description": "Primary intended behaviour"},
                {"axis": "edge", "description": "Boundary conditions"},
                {"axis": "integration", "description": "Interaction with adjacent features"},
                {"axis": "state", "description": "Side effects"},
            ],
            "bundle_hash": BUNDLE_HASH,
        }
    )


def build_bundle_for_runner_args_fallback() -> TcAuthorInput:
    """TC-288 — synthetic bundle where worker should fall back to sufficient."""
    return TcAuthorInput.model_validate(
        {
            "endpoint": _FIXTURE_ENDPOINT,
            "model_id": _FIXTURE_MODEL_ID,
            "feature_id": "FT-stub",
            "feature_spec": (
                "# FT-stub — example feature\n\n"
                "This is a synthetic feature for TC-288. The worker will try "
                "to propose TCs but fail runner_args validation twice."
            ),
            "existing_tcs": [],
            "tc_schema": {
                "required_fields": ["title", "type", "runner", "runner-args"],
                "allowed_types": ["scenario", "exit-criteria", "invariant"],
                "allowed_runners": ["bash", "cargo-test", "pytest", "custom"],
            },
            "runner_vocabulary": [
                {
                    "kind": "bash",
                    "args_pattern": r".+\.sh$",
                    "timeout_default": "60s",
                },
                {
                    "kind": "cargo-test",
                    "args_pattern": r"^[a-zA-Z_][a-zA-Z0-9_]*$",
                    "timeout_default": "120s",
                },
            ],
            "target_count": 3,
            "coverage_axes": [
                {"axis": "happy", "description": "Primary intended behaviour"},
            ],
            "bundle_hash": BUNDLE_HASH,
        }
    )


def build_bundle_for_protocol_check() -> TcAuthorInput:
    """TC-289 — any valid bundle works; the test mocks a wrong-hash response."""
    return TcAuthorInput.model_validate(
        {
            "endpoint": _FIXTURE_ENDPOINT,
            "model_id": _FIXTURE_MODEL_ID,
            "feature_id": "FT-stub",
            "feature_spec": "# FT-stub — bundle-hash echo check\n\nThe model is mocked to return a proposal with a stale bundle_hash.",
            "existing_tcs": [],
            "tc_schema": {
                "required_fields": ["title", "type", "runner", "runner-args"],
                "allowed_types": ["scenario", "exit-criteria", "invariant"],
                "allowed_runners": ["bash", "cargo-test", "pytest", "custom"],
            },
            "runner_vocabulary": [
                {
                    "kind": "bash",
                    "args_pattern": r".+\.sh$",
                    "timeout_default": "60s",
                }
            ],
            "target_count": 2,
            "coverage_axes": [{"axis": "happy", "description": "Primary intended behaviour"}],
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
    """Replace the live router builder with a sentinel that fails if invoked.

    This helper patches the router-building entry point so any test that
    forgets to inject a caller surfaces a clear AssertionError instead of
    attempting a live SDK call.
    """
    from tc_author import worker as worker_mod

    def boom(*args, **kwargs):  # noqa: ARG001
        raise AssertionError(
            "live model call escaped the mock — test must inject a caller"
        )

    monkeypatch.setattr(worker_mod, "_build_router_for_bundle", boom)
