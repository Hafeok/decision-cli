"""Python code-writer worker for decision-cli's implementer role.

Stateless contract per ADR-008: bundle in, CodeChange artifact out.
Workers MUST NOT talk to the graph. The harness owns reads and writes.
"""

from .claude_runner import run_dispatch
from .models import (
    CodeChange,
    DispatchPayload,
    FileWrite,
    ToolCall,
    WorkerError,
    WorkerResponse,
    WorkerTelemetry,
)

__version__ = "0.1.0"

__all__ = [
    "CodeChange",
    "DispatchPayload",
    "FileWrite",
    "ToolCall",
    "WorkerError",
    "WorkerResponse",
    "WorkerTelemetry",
    "run_dispatch",
]
