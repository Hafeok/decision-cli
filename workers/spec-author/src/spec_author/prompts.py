"""Builds the spec-author system prompt plus per-bundle user/retry messages."""

from __future__ import annotations

from .bundle import SpecAuthorInput

SYSTEM_PROMPT = """\
You are the spec-author role in decision-cli's Decision-Driven Design pipeline.

Your single responsibility: given an originating request/brief, the repo's
H2/H3 body-schema contract (ADR-047), related feature_specs, central ADRs,
the domain registry, and your authority declaration (ADR-027), propose a
feature_spec body that:

  * Addresses the request faithfully.
  * Conforms to the H2/H3 body schema — every required H2 section appears,
    and every required H3 subsection appears under Functional Specification.
  * Is testable: every required behaviour maps to at least one observable
    condition.
  * Is bounded: the Out of scope section is non-empty and substantive.
  * Respects the central ADRs (does not contradict them).
  * Does not duplicate scope claimed by an existing related feature_spec.

You return exactly one SpecProposal, with one of two kinds:

  - "new": a complete, schema-conforming feature_spec body. Populate
    `new.title`, `new.body` (full markdown), `new.proposed_depends_on`,
    `new.proposed_adrs` (existing ADRs the spec links to), and
    `new.proposed_domains` (from the domain_registry). Write a `new.rationale`
    that explains why the body addresses the request and respects the
    central ADRs.

  - "gap": when the brief is too under-specified to produce a defensible
    spec — missing scope, contradictory constraints, no stated boundary,
    or any item in your authority's `must_escalate` list that is silent in
    the brief. Populate `gap.missing_information` with the concrete items
    the brief should clarify, and `gap.reason` with why authoring is not
    possible. DO NOT invent decisions that belong in ADRs.

Hard constraints (any violation invalidates the proposal):

  - `new.body` MUST contain every H2 section from body_schema.required_h2_sections.
    For each required H3 subsection, the Functional Specification H2 must
    contain a `### <subsection>` line with substantive content beneath it.
  - `new.body` MUST have a non-empty, substantive Out of scope section.
  - `new.rationale` MUST be at least 20 characters.
  - `gap.missing_information` MUST have at least one entry.
  - `gap.reason` MUST be non-empty.
  - `bundle_hash` MUST be the exact `bundle_hash` from the input bundle.
    Do not invent, redact, or modify it.
  - You do not have tool access. You judge from the bundle text alone.
  - Prefer "gap" over inventing architectural decisions. If the brief is
    silent on a scope question the body would have to answer, return a Gap.

Output a single JSON object that matches the SpecProposal schema exactly:
top-level keys `kind`, `bundle_hash`, and exactly one of `new` or `gap`
populated.
"""


def build_user_prompt(bundle: SpecAuthorInput) -> str:
    """Build the user message from the bundle."""
    parts: list[str] = []

    # Request brief verbatim.
    parts.append("# Originating Request\n\n")
    parts.append(f"**Request ID:** {bundle.request.id}\n")
    if bundle.request.title:
        parts.append(f"**Title:** {bundle.request.title}\n")
    if bundle.request.source:
        parts.append(f"**Source:** {bundle.request.source}\n")
    parts.append("\n")
    parts.append(bundle.request.body)
    parts.append("\n\n")

    if bundle.feature_id_hint:
        parts.append(f"**Feature ID hint:** {bundle.feature_id_hint}\n\n")

    # Body schema (H2/H3 contract).
    parts.append("# Body Schema (ADR-047)\n\n")
    parts.append("Required H2 sections (each MUST appear in `new.body`):\n\n")
    for section in bundle.body_schema.required_h2_sections:
        guidance = bundle.body_schema.section_descriptions.get(section, "")
        if guidance:
            parts.append(f"- `## {section}` — {guidance}\n")
        else:
            parts.append(f"- `## {section}`\n")
    parts.append("\n")

    parts.append(
        "Required H3 subsections under `## Functional Specification` "
        "(each MUST appear as `### <name>` beneath the H2):\n\n"
    )
    for subsection in bundle.body_schema.required_h3_subsections:
        guidance = bundle.body_schema.section_descriptions.get(subsection, "")
        if guidance:
            parts.append(f"- `### {subsection}` — {guidance}\n")
        else:
            parts.append(f"- `### {subsection}`\n")
    parts.append("\n")

    # Related features.
    parts.append("# Related Features\n\n")
    if bundle.related_features:
        for feat in bundle.related_features:
            parts.append(f"## {feat.id}: {feat.title}\n\n")
            if feat.description:
                parts.append(f"**Description:** {feat.description}\n\n")
            if feat.boundaries:
                parts.append(f"**Boundaries:** {feat.boundaries}\n\n")
    else:
        parts.append("None.\n\n")

    # Central ADRs.
    parts.append("# Central ADRs (must be respected)\n\n")
    if bundle.central_adrs:
        for adr in bundle.central_adrs:
            parts.append(f"## {adr.id}: {adr.title}\n\n")
            if adr.scope:
                parts.append(f"**Scope:** {adr.scope}\n\n")
            if adr.summary:
                parts.append(f"{adr.summary}\n\n")
    else:
        parts.append("None.\n\n")

    # Domain registry.
    parts.append("# Domain Registry (allowed `proposed_domains` values)\n\n")
    if bundle.domain_registry:
        for dom in bundle.domain_registry:
            if dom.description:
                parts.append(f"- **{dom.name}** — {dom.description}\n")
            else:
                parts.append(f"- **{dom.name}**\n")
    else:
        parts.append("(empty)\n")
    parts.append("\n")

    # Authority declaration.
    parts.append("# Your Authority (ADR-027)\n\n")
    if bundle.authority.may_decide:
        parts.append("**You MAY decide unilaterally:**\n\n")
        for cat in bundle.authority.may_decide:
            parts.append(f"- {cat}\n")
        parts.append("\n")
    if bundle.authority.must_escalate:
        parts.append("**You MUST escalate via Gap (do not invent):**\n\n")
        for cat in bundle.authority.must_escalate:
            parts.append(f"- {cat}\n")
        parts.append("\n")
    if bundle.authority.rationale:
        parts.append(f"_Rationale: {bundle.authority.rationale}_\n\n")

    # Bundle metadata.
    parts.append("# Bundle Metadata\n\n")
    parts.append(
        f"**bundle_hash (echo this verbatim in your proposal):** {bundle.bundle_hash}\n"
    )
    parts.append(f"**endpoint:** {bundle.endpoint}\n")
    parts.append(f"**model_id:** {bundle.model_id}\n")
    parts.append("\n")

    return "".join(parts)


def build_retry_prompt(validation_error: str) -> str:
    """Build a retry message after schema validation failure."""
    return (
        "Your previous response failed validation:\n\n"
        f"{validation_error}\n\n"
        "Please correct the errors and output a valid SpecProposal JSON object. "
        "If you cannot produce a schema-conformant body, return `kind: gap` with "
        "missing_information describing the obstacle."
    )
