"""Python verify-graph-author worker for decision-cli's graph-proposal role.

Stateless contract per ADR-008 / ADR-030: bundle in, GraphProposal
artifact out. Single-shot LLM call, no tool use, no graph access. The
harness owns reads and writes; the worker consumes only the bundle it
is handed.
"""

from .bundle import (
    EnvRecord,
    ExistingGraphRecord,
    StepKindRecord,
    StepSummary,
    TcRecord,
    VerifyGraphAuthorInput,
)
from .output import (
    GapProposal,
    GraphProposal,
    MatchProposal,
    NewProposal,
    ProposedStep,
)

__version__ = "0.1.0"

__all__ = [
    "EnvRecord",
    "ExistingGraphRecord",
    "GapProposal",
    "GraphProposal",
    "MatchProposal",
    "NewProposal",
    "ProposedStep",
    "StepKindRecord",
    "StepSummary",
    "TcRecord",
    "VerifyGraphAuthorInput",
]
