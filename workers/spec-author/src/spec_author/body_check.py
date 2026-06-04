"""Body-completeness validator for spec-author proposals.

Mirrors the harness-side check (FT-055, ADR-047) so a freshly authored
spec lands without W030 warnings. The worker validates structurally
before exit; the harness re-validates on ingest.
"""

from __future__ import annotations

from dataclasses import dataclass, field


@dataclass
class BodyCheckResult:
    """Structured result of validating a feature_spec body."""

    errors: list[str] = field(default_factory=list)
    warnings: list[str] = field(default_factory=list)
    h2_present: set[str] = field(default_factory=set)
    h3_present: set[str] = field(default_factory=set)

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


def _h3_headings(body: str) -> set[str]:
    """Set of H3 heading titles (after `### `) in the body."""
    return {
        line[4:].strip()
        for line in body.splitlines()
        if line.startswith("### ")
    }


def check_body_completeness(
    body: str,
    required_h2: list[str],
    required_h3: list[str],
) -> BodyCheckResult:
    """Validate that ``body`` contains every required H2 and H3 section.

    Returns a BodyCheckResult listing any missing sections. Warnings use
    the W030 code so the harness's ingest gate can recognise the same
    failure mode. An empty result means the body is conformant.
    """
    result = BodyCheckResult()
    h2 = _h2_headings(body)
    h3 = _h3_headings(body)
    result.h2_present = h2
    result.h3_present = h3

    for section in required_h2:
        if section not in h2:
            result.warnings.append(
                f"W030 missing required H2 section: `## {section}`"
            )

    for subsection in required_h3:
        if subsection not in h3:
            result.warnings.append(
                f"W030 missing required H3 subsection: `### {subsection}`"
            )

    return result
