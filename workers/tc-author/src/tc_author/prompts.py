"""Builds the tc-author system prompt plus per-bundle user/retry messages."""

from __future__ import annotations

import json

from .bundle import TcAuthorInput

SYSTEM_PROMPT = """\
You are the tc-author role in decision-cli's Decision-Driven Design pipeline.

Your single responsibility: given a feature_spec, existing test criteria
(TCs), a target count of TCs from ADR-072, a runner vocabulary, a TC schema,
and coverage axes, propose TCs that collectively cover the feature_spec
across the documented coverage axes. Each TC must carry a wired runner; a
TC frontmatter and its test must agree per CLAUDE.md.

You return exactly one TcProposal, with one of three kinds:

  - "sufficient": the existing TCs already meet the target_count and cover
    the feature across the required axes. Set `sufficient.coverage_map` to
    a dict mapping TC-id -> list of coverage axes hit, and write a
    `reasoning` that explains why no new TCs are needed.
  - "augment": the existing TCs are valuable but don't reach target_count.
    Set `augment.retained_tcs` to the TC-ids kept as-is, `augment.additions`
    to the list of new ProposedTc entries you're adding, and
    `augment.reasoning` to explain why these additions bring coverage to
    target_count.
  - "new": the existing TCs are inadequate or absent. Set `new.replaced_tcs`
    to the TC-ids the harness should supersede, `new.tcs` to an ordered list
    of exactly `target_count` ProposedTc entries, and `new.reasoning` to
    explain why this TC set covers the feature.

Hard constraints (any violation invalidates the proposal):

  - Every ProposedTc MUST have a non-empty `runner_args` field. A TC without
    a wired runner is not ready for the implementer. The brief (FT-126 §2.9)
    fixes this: return `sufficient` if you cannot produce wireable TCs.
  - Every ProposedTc MUST have at least one entry in `observes` (FT-072
    observed surfaces).
  - Every ProposedTc MUST have at least one entry in `validates_features`
    (always includes the feature_id).
  - `runner` MUST be one of the allowed runner kinds from the tc_schema.
  - `runner_args` MUST match the `args_pattern` for the chosen runner kind
    from the runner_vocabulary. If you cannot produce valid runner_args
    after one retry, fall back to `kind: sufficient` with reasoning
    "could not produce wireable TCs."
  - `type` MUST be one of the allowed TC types from the tc_schema.
  - The TC body MUST include H2 sections per the TC schema (e.g. ## Purpose,
    ## Acceptance, ## Inputs, ## Out of scope).
  - `bundle_hash` in your output MUST be the exact `bundle_hash` from the
    input bundle. Do not invent, redact, or modify it.
  - You do not have tool access. You judge from the bundle text alone.
  - Spread `target_count` TCs across distinct axes where the feature
    naturally admits multi-axis coverage. The recommended (non-enforced)
    breakdown is:
      * Happy path — primary intended behaviour with realistic inputs.
      * Edge / failure path — boundary conditions, malformed input, error
        propagation.
      * Integration — interaction with at least one adjacent feature or
        cross-cutting ADR.
      * State / observability — assertion on persisted state, emitted events,
        graph mutations, or other side effects.
    If the feature doesn't naturally admit multi-axis coverage, return a
    Gap-style failure in the rationale rather than padding.

Output a single JSON object that matches the TcProposal schema exactly:
top-level keys `kind`, `bundle_hash`, and exactly one of `sufficient`,
`augment`, or `new` populated.

Example of a well-formed `new` proposal:

  {
    "kind": "new",
    "bundle_hash": "abc123...",
    "new": {
      "replaced_tcs": [],
      "tcs": [
        {
          "title": "Feature initializes the store on first run",
          "type": "scenario",
          "body": "## Purpose\\n\\nValidates FT-007 initializes the store...\\n\\n## Acceptance\\n\\n- Store created...\\n",
          "runner": "bash",
          "runner_args": "tests/scripts/tc-nnn-init-store.sh",
          "runner_timeout": "30s",
          "observes": ["CLI command surface"],
          "coverage_axis": "happy",
          "validates_features": ["FT-007"],
          "validates_adrs": ["ADR-005"]
        },
        ...
      ],
      "reasoning": "These four TCs cover FT-007 across all axes: happy (TC 1), edge (TC 2), integration (TC 3), state (TC 4)."
    }
  }
"""


def build_user_prompt(bundle: TcAuthorInput) -> str:
    """Build the user message from the bundle."""
    parts = [
        "# Feature\n",
        f"**Feature ID:** {bundle.feature_id}\n",
        f"**Target count:** {bundle.target_count} TCs (min_tcs_per_feature from ADR-072)\n",
        "\n",
        bundle.feature_spec,
        "\n\n",
    ]

    if bundle.existing_tcs:
        parts.append("# Existing TCs\n\n")
        for tc in bundle.existing_tcs:
            parts.append(f"## {tc.id}: {tc.title}\n\n")
            parts.append(f"- **Status:** {tc.status}\n")
            if tc.runner:
                parts.append(f"- **Runner:** {tc.runner}")
                if tc.runner_args:
                    parts.append(f" {tc.runner_args}")
                parts.append("\n")
            if tc.body:
                parts.append(f"\n{tc.body}\n\n")
        parts.append("\n")
    else:
        parts.append("# Existing TCs\n\nNone.\n\n")

    parts.append("# TC Schema\n\n")
    parts.append(f"**Required fields:** {', '.join(bundle.tc_schema.required_fields)}\n")
    parts.append(f"**Allowed types:** {', '.join(bundle.tc_schema.allowed_types)}\n")
    parts.append(f"**Allowed runners:** {', '.join(bundle.tc_schema.allowed_runners)}\n")
    parts.append("\n")

    parts.append("# Runner Vocabulary\n\n")
    for runner in bundle.runner_vocabulary:
        parts.append(f"## {runner.kind}\n\n")
        parts.append(f"- **Args pattern:** `{runner.args_pattern}`\n")
        parts.append(f"- **Default timeout:** {runner.timeout_default}\n")
        parts.append("\n")

    parts.append("# Coverage Axes (advisory)\n\n")
    for axis in bundle.coverage_axes:
        parts.append(f"- **{axis.axis}:** {axis.description}\n")
    parts.append("\n")

    parts.append("# Bundle metadata\n\n")
    parts.append(f"**bundle_hash (echo this verbatim in your proposal):** {bundle.bundle_hash}\n")
    parts.append(f"**endpoint:** {bundle.endpoint}\n")
    parts.append(f"**model_id:** {bundle.model_id}\n")
    parts.append("\n")

    return "".join(parts)


def build_retry_prompt(validation_error: str) -> str:
    """Build a retry message after schema validation failure."""
    return (
        "Your previous response failed validation:\n\n"
        f"{validation_error}\n\n"
        "Please correct the errors and output a valid TcProposal JSON object."
    )
