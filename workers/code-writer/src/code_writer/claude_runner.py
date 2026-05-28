"""Invoke Claude Code in headless mode (``claude -p``) and observe writes.

This module implements ADR-008 §Behaviour step 3-5 for FT-013:

1. Spawn ``claude -p`` with ``--output-format stream-json`` so tool-call
   and result events surface deterministically.
2. Pipe the bundle markdown to stdin as the prompt.
3. Stream stdout, parsing JSONL events into per-turn telemetry and a
   final result block.
4. Build a :class:`CodeChange` from the observed ``Edit`` / ``Write``
   tool calls and the model's final summary.

A "stub mode" (controlled by the ``CODE_WRITER_STUB=1`` env var or
``--stub`` flag) bypasses the ``claude`` subprocess entirely. The stub
writes a deterministic marker file inside the workspace and returns a
synthetic :class:`CodeChange` so the harness end-to-end flow can be
exercised in CI without depending on a Claude subscription. This is the
single switch every TC needs.
"""

from __future__ import annotations

from ._runner_common import (
    STUB_ENV_VAR,
    _claude_on_path,
    _is_stub_mode,
    _make_code_change_iri,
    _safe_join,
)
from ._stub_runner import StubResult, _default_stub_result, run_stub
from .models import DispatchPayload, WorkerResponse

__all__ = [
    "STUB_ENV_VAR",
    "StubResult",
    "run_claude",
    "run_dispatch",
    "run_stub",
]

# The private helpers (``_claude_on_path``, ``_safe_join`` …) are kept as
# attributes of this module so existing test code that mocks
# ``code_writer.claude_runner._claude_on_path`` keeps working after the
# split into ``_runner_common`` / ``_stub_runner`` / ``_subprocess_runner``.
_ = (_claude_on_path, _is_stub_mode, _make_code_change_iri, _safe_join, _default_stub_result)


def run_claude(payload: DispatchPayload) -> WorkerResponse:
    """Real ``claude -p`` subprocess runner (ADR-008 §Behaviour 3-5)."""
    from ._subprocess_runner import run_claude as _impl

    return _impl(payload)


def run_dispatch(
    payload: DispatchPayload, *, force_stub: bool | None = None
) -> WorkerResponse:
    """Single dispatch entry point — picks stub vs. real runner.

    ``force_stub`` overrides the env var (used by the one-shot CLI's
    ``--stub`` flag). When ``None`` (the default), the env var
    ``CODE_WRITER_STUB`` is consulted.
    """
    use_stub = _is_stub_mode() if force_stub is None else force_stub
    if use_stub:
        return run_stub(payload)
    return run_claude(payload)
