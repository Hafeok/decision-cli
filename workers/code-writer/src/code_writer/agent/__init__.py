"""In-process LiteLLM-client agentic loop for code-writer (FT-123 / ADR-069).

Public API:
    run_agent(payload: DispatchPayload) -> WorkerResponse
"""

from __future__ import annotations

from .loop import run_agent

__all__ = ["run_agent"]
