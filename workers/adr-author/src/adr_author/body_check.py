"""Body-completeness validator for adr-author proposals.

Mirrors the H2-only contract for ADR bodies. An ADR body MUST contain
every required H2 section (Context, Decision, Rejected alternatives,
Consequences, Status) for the worker to consider it a valid `new`
proposal. The harness re-validates on ingest.
"""

from __future__ import annotations

from dataclasses import dataclass, field


@dataclass
class BodyCheckResult:
    """Structured result of validating an ADR body."""

    errors: list[str] = field(default_factory=list)
    warnings: list[str] = field(default_factory=list)
    h2_present: set[str] = field(default_factory=set)

    @property
    def passes(self) -> bool:
        return not self.errors and not self.warnings


def _h2_headings(body: str) -> set[str]:
    """Set of H2 heading titles (after `## `) in the body."""
    return {
        line[3:].strip()
        for line in body.splitlines()
        if line.startswith("## ")
    }


def check_body_completeness(
    body: str,
    required_h2: list[str],
) -> BodyCheckResult:
    """Validate that ``body`` contains every required H2 section.

    Returns a ``BodyCheckResult`` listing any missing sections. An
    empty result means the body is conformant.
    """
    result = BodyCheckResult()
    h2 = _h2_headings(body)
    result.h2_present = h2

    for section in required_h2:
        if section not in h2:
            result.warnings.append(
                f"missing required H2 section: `## {section}`"
            )

    return result
