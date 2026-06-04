"""System prompt rendering and FT-108 feedback prefix logic (FT-123)."""

from __future__ import annotations

import json

from ..models import DefectFeedbackRecord, DispatchPayload

# FT-108: agent must end its final result with a marker-delimited JSON
# block whose `iris` field lists every feedback IRI from the bundle's
# `defect_feedback` array that this code change addresses.
ADDRESSED_FEEDBACK_BEGIN = "<<DEC_ADDRESSED_FEEDBACK>>"
ADDRESSED_FEEDBACK_END = "<<END_DEC_ADDRESSED_FEEDBACK>>"


def render_system_prompt(payload: DispatchPayload) -> str:
    """Render the full system prompt for the dispatch.

    When the bundle carries defect feedback (FT-108), the prompt is
    PREFIXED (not suffixed) with the feedback section + citation block
    requirement. Prefixing matters: bundles can be 100K+ tokens, and
    LLMs read top-down — instructions placed after the bundle get lost
    in implementation thinking before they're reached.

    Args:
        payload: The dispatch payload with bundle and optional feedback.

    Returns:
        The complete system prompt text.
    """
    if payload.defect_feedback:
        return (
            f"{_render_defect_feedback_section(payload.defect_feedback)}\n\n"
            f"---\n\n"
            f"{payload.bundle_markdown}"
        )
    return payload.bundle_markdown


def _render_defect_feedback_section(records: list[DefectFeedbackRecord]) -> str:
    """Render the FT-108 defect-feedback section for the agent prompt.

    Renders an "outcome contract" first (read-this-first heading + the
    exact citation block format the server-side extractor expects),
    then the individual feedback entries.
    """
    iris_json = json.dumps([r.feedback_iri for r in records], indent=2)
    empty_block = ADDRESSED_FEEDBACK_BEGIN + '\n{ "iris": [] }\n' + ADDRESSED_FEEDBACK_END
    lines = [
        "# ⚠ READ FIRST — Runtime defect feedback (FT-108)",
        "",
        f"This dispatch carries {len(records)} runtime defect(s) the prior verifier",
        "produced against this feature's tests. **The orchestrator REQUIRES** that",
        "your final assistant message end with a citation block listing every",
        "feedback IRI your code change actually addressed. Without it the dispatch",
        "is rejected and the feedback stays open — which makes the driver loop",
        "report no-progress and escalate.",
        "",
        "## The citation block — EXACT format",
        "",
        "Your final assistant message must contain this verbatim block, with the",
        "marker strings EXACTLY as shown (no whitespace variation, no commentary",
        "inside the markers):",
        "",
        "```",
        ADDRESSED_FEEDBACK_BEGIN,
        "{",
        f'  "iris": {_indent_json(iris_json, 2)}',
        "}",
        ADDRESSED_FEEDBACK_END,
        "```",
        "",
        "The `iris` array lists every IRI you fixed. Drop any you couldn't fix.",
        "",
        "## What counts as \"addressed\"",
        "",
        "An IRI is addressed when the code change you produced would make the",
        "evidence go away on a fresh verify run. Renaming an unrelated function,",
        "adding a test stub that doesn't run, or writing a comment near the broken",
        "code does NOT count as addressed. Cite ONLY the IRIs whose underlying",
        "issue your diff actually fixes.",
        "",
        "## If you can't address any of them",
        "",
        "If after inspecting the bundle and the defects you conclude that NONE of",
        "the defects describe a real code issue (e.g., they're all spec gaps, or",
        "describe behaviour outside this feature's scope, or the underlying tests",
        "are themselves wrong), emit the citation block with an EMPTY array and",
        "explain in plain text BEFORE the block which defects you couldn't",
        "address and why. The driver loop reads the empty array as an explicit",
        "no-op signal and escalates to spec-author — this is far better than a",
        "missing citation block, which the server treats as a malformed",
        "dispatch.",
        "",
        "Empty-array form:",
        "",
        "```",
        empty_block,
        "```",
        "",
        "## The defects",
        "",
    ]
    for r in records:
        lines.append(f"### {r.feedback_iri}")
        if r.source_tc:
            lines.append(f"- source TC: `{r.source_tc}`")
        if r.graph_id:
            lines.append(
                f"- source graph: `.dec/verify/graph/{r.graph_id}.ttl` — "
                "**read this file first** to see the exact command that "
                "failed; the `evidence` line below only carries the runner's "
                "one-line diagnostic, not the full step text"
            )
        lines.append(f"- severity: {r.severity}")
        lines.append(f"- evidence: {r.evidence.strip() or '(empty)'}")
        lines.append("")
    return "\n".join(lines)


def _indent_json(blob: str, indent_spaces: int) -> str:
    """Indent every line of a JSON blob past the first by `indent_spaces`."""
    pad = " " * indent_spaces
    lines = blob.splitlines()
    if not lines:
        return blob
    return lines[0] + "\n" + "\n".join(pad + line for line in lines[1:])


def render_user_message(payload: DispatchPayload) -> str:
    """Render the initial user message for the dispatch.

    Args:
        payload: The dispatch payload.

    Returns:
        The user message text.
    """
    has_defects = bool(payload.defect_feedback)
    if has_defects:
        return (
            f"Implement feature {payload.feature_id} described in the system prompt.\n\n"
            "The bundle's `# ⚠ READ FIRST` section at the top lists runtime defects from "
            "prior verification runs. For each defect, the listed `source graph` path "
            "(`.dec/verify/graph/VG-NNN.ttl`) shows the exact step text that failed — "
            "READ THAT FILE before deciding what to change. The defect's `evidence` line "
            "is the runner's one-line summary, not the whole story.\n\n"
            "When the failing step exercises a `dec ...` subcommand, the fix lives in the "
            "Rust source under `crates/decision-cli/` (or `crates/oxi-events/`); after "
            "editing it you MUST run `cargo install --path crates/decision-cli --bin dec --offline` "
            "so the verifier's next dispatch picks up your new binary. When it exercises a "
            "worker (`code-writer`, `verify-graph-author`), edit the Python source under "
            "`workers/<name>/` and re-install with `uv tool install workers/<name> --reinstall`.\n\n"
            "If the failing step's command itself is wrong (the graph asks for a command "
            "that was never meant to exist, or tests behaviour out of this feature's scope), "
            "emit the citation block with `{\"iris\": []}` and explain why in plain text — "
            "the driver will route those defects to the verify-graph-author to re-author the "
            "test instead.\n\n"
            "Run `product verify` when done."
        )
    return (
        f"Implement feature {payload.feature_id} described in the system "
        "prompt. Follow all constraints and run `product verify` when done."
    )
