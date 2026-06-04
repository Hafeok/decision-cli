"""spec-author worker package for decision-cli's feature_spec authoring role (ADR-073, FT-129)."""

__version__ = "0.1.0"

from .bundle import SpecAuthorInput
from .output import GapProposal, NewSpecProposal, SpecProposal
from .worker import WorkerError, WorkerResult, run_author

__all__ = [
    "GapProposal",
    "NewSpecProposal",
    "SpecAuthorInput",
    "SpecProposal",
    "WorkerError",
    "WorkerResult",
    "run_author",
]
