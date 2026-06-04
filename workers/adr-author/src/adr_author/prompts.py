"""Builds the adr-author system prompt plus per-bundle user/retry messages."""

from __future__ import annotations

from .bundle import AdrAuthorInput

SYSTEM_PROMPT = """\
You are the adr-author role in decision-cli's Decision-Driven Design pipeline.

Your single responsibility: given a preflight gap (an unacknowledged
cross-cutting ADR or an uncovered domain), the feature_spec that
surfaced the gap, the graph-central ADRs, the ADR body schema, the
domain registry, and your authority declaration (ADR-027), produce one
of three structured proposals:

  - "new": author a complete, schema-conforming net-new ADR when the
    gap warrants a fresh decision and no existing ADR governs the
    feature. Populate `new.title`, `new.body` (full markdown), `new.scope`
    (one of: cross-cutting, platform, domain, feature-specific),
    `new.proposed_domains`, `new.addresses_gap` (echo the preflight_gap
    payload verbatim), and `new.rationale`.

  - "acknowledgement": when an existing central ADR already governs the
    feature and the gap exists because the spec failed to link it.
    Populate `acknowledgement.acknowledges` (the ADR id or domain name),
    `acknowledgement.target_feature` (the feature id), `acknowledgement.reasoning`
    (a substantive explanation of how the existing ADR governs the
    feature — MUST be at least 40 characters), and `acknowledgement.rationale`
    (why this acknowledgement closes the gap).

  - "gap": when the brief is too under-specified to author a defensible
    ADR AND no existing ADR plausibly governs the feature. Populate
    `gap.missing_information` with concrete items the brief should
    clarify, and `gap.reason` with why authoring is not possible. DO NOT
    invent decisions that belong in ADRs.

Hard constraints (any violation invalidates the proposal):

  - `new.body` MUST contain every H2 section from
    adr_body_schema.required_h2_sections with substantive content.
  - `new.scope` MUST be exactly one of: cross-cutting, platform, domain,
    feature-specific.
  - `new.addresses_gap` MUST echo the preflight_gap payload verbatim.
  - `new.rationale` MUST be at least 20 characters.
  - `acknowledgement.reasoning` MUST be at least 40 characters of
    substantive prose. A bare acknowledgement (empty, whitespace-only,
    or trivially short reasoning) is FORBIDDEN. If you cannot produce
    substantive reasoning, return `kind: gap` instead.
  - `acknowledgement.acknowledges` MUST reference an ADR id present in
    the central_adrs list or a domain name from domain_registry.
  - `gap.missing_information` MUST have at least one entry.
  - `gap.reason` MUST be non-empty.
  - `bundle_hash` MUST be the exact `bundle_hash` from the input bundle.
    Do not invent, redact, or modify it.
  - You do not have tool access. You judge from the bundle text alone.
  - Prefer "gap" over inventing architectural decisions. If neither a
    net-new ADR nor a reasoned acknowledgement is defensible, return a
    Gap rather than producing a bare or fabricated acknowledgement.

Output a single JSON object that matches the AdrProposal schema exactly:
top-level keys `kind`, `bundle_hash`, and exactly one of `new`,
`acknowledgement`, or `gap` populated.
"""


def build_user_prompt(bundle: AdrAuthorInput) -> str:
    """Assemble the user message from the bundle by concatenating sections."""
    sections = [
        _section_goal(),
        _section_feature(bundle),
        _section_gap(bundle),
        _section_central_adrs(bundle),
        _section_body_schema(bundle),
        _section_domain_registry(bundle),
        _section_authority(bundle),
        _section_metadata(bundle),
    ]
    return "".join(sections)


def _section_goal() -> str:
    return (
        "# Goal\n\n"
        "Address the preflight gap. Choose: author a NEW ADR if the gap "
        "warrants a new decision; AUTHOR an acknowledgement with reasoning "
        "if an existing ADR governs the feature and the acknowledgement "
        "closes the gap; emit GAP if the brief under-specifies the "
        "decision space.\n\n"
    )


def _section_feature(bundle: AdrAuthorInput) -> str:
    parts = [f"# Feature: {bundle.feature_id}\n\n"]
    if bundle.feature_spec.strip():
        parts.append(bundle.feature_spec)
        parts.append("\n\n")
    else:
        parts.append("(feature_spec body was not provided)\n\n")
    return "".join(parts)


def _section_gap(bundle: AdrAuthorInput) -> str:
    gap = bundle.preflight_gap
    parts = ["# Preflight Gap\n\n", f"**kind:** {gap.kind}\n"]
    if gap.adr_id:
        parts.append(f"**adr_id:** {gap.adr_id}\n")
    if gap.domain:
        parts.append(f"**domain:** {gap.domain}\n")
    parts.append(f"**severity:** {gap.severity}\n")
    if gap.message:
        parts.append(f"**message:** {gap.message}\n")
    parts.append("\n")
    return "".join(parts)


def _section_central_adrs(bundle: AdrAuthorInput) -> str:
    parts = ["# Central ADRs (use as governing context)\n\n"]
    if not bundle.central_adrs:
        parts.append("None.\n\n")
        return "".join(parts)
    for adr in bundle.central_adrs:
        parts.append(f"## {adr.id}: {adr.title}\n\n")
        if adr.scope:
            parts.append(f"**Scope:** {adr.scope}\n\n")
        if adr.summary:
            parts.append(f"{adr.summary}\n\n")
    return "".join(parts)


def _section_body_schema(bundle: AdrAuthorInput) -> str:
    parts = [
        "# ADR Body Schema\n\n",
        "Required H2 sections (each MUST appear in `new.body` with "
        "substantive content):\n\n",
    ]
    for section in bundle.adr_body_schema.required_h2_sections:
        guidance = bundle.adr_body_schema.section_descriptions.get(section, "")
        if guidance:
            parts.append(f"- `## {section}` — {guidance}\n")
        else:
            parts.append(f"- `## {section}`\n")
    parts.append("\n")
    return "".join(parts)


def _section_domain_registry(bundle: AdrAuthorInput) -> str:
    parts = ["# Domain Registry (allowed `proposed_domains` values)\n\n"]
    if not bundle.domain_registry:
        parts.append("(empty)\n\n")
        return "".join(parts)
    for dom in bundle.domain_registry:
        if dom.description:
            parts.append(f"- **{dom.name}** — {dom.description}\n")
        else:
            parts.append(f"- **{dom.name}**\n")
    parts.append("\n")
    return "".join(parts)


def _section_authority(bundle: AdrAuthorInput) -> str:
    parts = ["# Your Authority (ADR-027)\n\n"]
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
    parts.append(
        "**Bare acknowledgement rule (FT-130 §4B):** if you choose "
        "`acknowledgement`, the `reasoning` field MUST be at least 40 "
        "characters of substantive prose. An empty or whitespace-only "
        "`reasoning` is forbidden. If you cannot defend a substantive "
        "acknowledgement, emit `gap` instead.\n\n"
    )
    if bundle.authority.rationale:
        parts.append(f"_Rationale: {bundle.authority.rationale}_\n\n")
    return "".join(parts)


def _section_metadata(bundle: AdrAuthorInput) -> str:
    return (
        "# Bundle Metadata\n\n"
        f"**bundle_hash (echo this verbatim in your proposal):** {bundle.bundle_hash}\n"
        f"**endpoint:** {bundle.endpoint}\n"
        f"**model_id:** {bundle.model_id}\n"
        "\n"
    )


def build_retry_prompt(validation_error: str) -> str:
    """Build a retry message after schema validation failure."""
    return (
        "Your previous response failed validation:\n\n"
        f"{validation_error}\n\n"
        "Please correct the errors and output a valid AdrProposal JSON object. "
        "If you cannot produce a defensible ADR or a substantive "
        "(≥ 40 character) acknowledgement, return `kind: gap` with "
        "missing_information describing the obstacle. NEVER emit an "
        "acknowledgement with empty or whitespace-only reasoning."
    )
