#!/usr/bin/env python3
"""Coherence audit for the `add-author-worker` TaskType (FT-140 / ADR-080).

Discriminator vs `add-judge-worker`: an author cluster's Output type
MUST carry `body_markdown: str` and MUST NOT carry a `verdict` field
in any spelling. A judge cluster's Output has `verdict: str`; an
author's does not. Fail-loud with the discriminator hint so
misclassification surfaces immediately.

Exit codes (per ADR-013):
  0 — every check passed.
  1 — at least one check failed; stderr explains which.
  2 — fixture unrunnable.
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

CELL_FILES = {
    "capability_binding": "capability_binding.nq",
    "pydantic_io_models": "pydantic_io_models.py",
    "system_prompt": "system_prompt.md",
    "agent_loop": "agent_loop.py",
    "unit_tests": "unit_tests.py",
}

VERDICT_SPELLINGS = ("verdict", "Verdict", "verdict_label", "VerdictLabel")


def die(check: str, detail: str, code: int = 1) -> None:
    sys.stderr.write(f"FAIL check={check}: {detail}\n")
    sys.exit(code)


def read(p: Path) -> str:
    try:
        return p.read_text(encoding="utf-8")
    except OSError as e:
        die("missing_file", f"{p}: {e}", code=2)
        return ""


def check_agent_loop_litellm_canonical(fixture: Path) -> None:
    body = read(fixture / CELL_FILES["agent_loop"])
    if not re.search(r"litellm\.completion\s*\(", body):
        die("agent_loop_calls_litellm_canonical", "no litellm.completion call")
    if not re.search(r"model\s*=\s*\S*model_id\b", body):
        die("agent_loop_calls_litellm_canonical", "no model=payload.model_id")
    if not re.search(r"base_url\s*=", body):
        die("agent_loop_calls_litellm_canonical", "no base_url=")


def check_output_is_draft_not_verdict(fixture: Path) -> None:
    """Load-bearing discriminator vs add-judge-worker. Verdict-field
    presence is the stronger signal — checked first so the
    misclassification hint surfaces before lesser shape complaints."""
    body = read(fixture / CELL_FILES["pydantic_io_models"])
    for spelling in VERDICT_SPELLINGS:
        if re.search(rf"^\s*{spelling}\s*:\s*", body, re.MULTILINE):
            die(
                "output_is_draft_not_verdict",
                f"Output has {spelling!r} field — "
                f"output is a verdict, not a draft — did you mean add-judge-worker?",
            )
    if not re.search(r"^\s*body_markdown\s*:\s*str", body, re.MULTILINE):
        die(
            "output_is_draft_not_verdict",
            "Output type lacks body_markdown: str — author clusters produce drafts",
        )


def check_output_schema_has_body_and_sections(fixture: Path) -> None:
    body = read(fixture / CELL_FILES["pydantic_io_models"])
    if not re.search(r"^\s*body_markdown\s*:\s*str", body, re.MULTILINE):
        die("output_schema_has_body_and_sections", "missing body_markdown: str")
    # `sections` field exists with a dict-like type (dict[..], Mapping[..]).
    if not re.search(
        r"^\s*sections\s*:\s*(dict|Mapping|Dict)\s*\[", body, re.MULTILINE
    ):
        die(
            "output_schema_has_body_and_sections",
            "missing sections: dict[str, str] | Mapping[str, str]",
        )


def main() -> int:
    if len(sys.argv) != 2:
        sys.stderr.write("usage: cluster-audit-add-author-worker.py <fixture_dir>\n")
        return 2
    fixture = Path(sys.argv[1])
    if not fixture.is_dir():
        sys.stderr.write(f"fixture {fixture!r} is not a directory\n")
        return 2
    check_agent_loop_litellm_canonical(fixture)
    check_output_is_draft_not_verdict(fixture)
    check_output_schema_has_body_and_sections(fixture)
    print("PASS add-author-worker (3 checks passed)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
