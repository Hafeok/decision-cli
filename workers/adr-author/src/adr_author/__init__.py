"""adr-author worker package for decision-cli's ADR authoring role (ADR-073, FT-130)."""

__version__ = "0.1.0"

from .bundle import AdrAuthorInput, PreflightGapRecord
from .output import (
    AcknowledgementProposal,
    AdrProposal,
    GapProposal,
    NewAdrProposal,
)
from .worker import WorkerError, WorkerResult, run_author

__all__ = [
    "AcknowledgementProposal",
    "AdrAuthorInput",
    "AdrProposal",
    "GapProposal",
    "NewAdrProposal",
    "PreflightGapRecord",
    "WorkerError",
    "WorkerResult",
    "run_author",
]
