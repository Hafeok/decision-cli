"""Python verifier worker for decision-cli's interpretation role.

Stateless contract per ADR-008 / ADR-020: bundle in, VerificationVerdict
artifact out. Single-shot LLM call, no tool use, no graph access. The
harness owns reads and writes; the verifier consumes only the bundle it
is handed.
"""

from .bundle import AdrRef, TcRef, VerifierInput
from .output import VerificationVerdict, Verdict

__version__ = "0.1.0"

__all__ = [
    "AdrRef",
    "TcRef",
    "Verdict",
    "VerificationVerdict",
    "VerifierInput",
]
