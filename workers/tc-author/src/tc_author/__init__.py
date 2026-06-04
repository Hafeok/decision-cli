"""tc-author worker package for decision-cli's TC authoring role (ADR-073, FT-126)."""

__version__ = "0.1.0"

from .bundle import TcAuthorInput
from .output import TcProposal
from .worker import WorkerError, WorkerResult, run_author

__all__ = [
    "TcAuthorInput",
    "TcProposal",
    "WorkerError",
    "WorkerResult",
    "run_author",
]
