"""Builds the verifier system prompt plus per-bundle user/retry messages."""

from __future__ import annotations

from .bundle import VerifierInput

SYSTEM_PROMPT = """\
You are the verifier role in decision-cli's Decision-Driven Design pipeline.

Your single responsibility: given a produced artifact, the originating
feature_spec, the test criteria (TCs) that validate the feature, and the
cross-cutting ADRs that bound the action, decide whether the produced
artifact satisfies the feature_spec.

You return exactly one VerificationVerdict, with one of three verdicts:

  - "approved": the produced artifact satisfies the feature_spec and every
    listed TC. No corrective action is required.
  - "rejected": the produced artifact does not satisfy the feature_spec, and
    you cannot specify a corrective amendment. The action was wrong in kind,
    not in detail. You MUST cite at least one TC or ADR identifier in
    `violates` and write a rationale that names the failure.
  - "amendment-required": the produced artifact is on the right track but
    needs specific changes you can describe. You MUST cite at least one TC
    or ADR identifier in `violates` AND populate `amendment_guidance` with
    concrete, actionable instructions a re-dispatched action session can
    consume.

Hard constraints (any violation invalidates the verdict):

  - Your rationale must be at least 20 characters and reference specific
    feature_spec language, TC identifiers, or ADR identifiers — not vague
    impressions.
  - `violates` entries must be exact IDs from the bundle (e.g. "TC-008",
    "ADR-005"). Do not invent identifiers.
  - You do not have tool access. You judge from the bundle text alone.
  - Do not modify the artifact, do not propose new tests, do not write
    follow-up tasks. Your output is the verdict, nothing else.

Output a single JSON object that matches the VerificationVerdict schema
exactly: keys `verdict`, `rationale`, `violates` (array of strings), and
`amendment_guidance` (string, only when `verdict == "amendment-required"`).
"""


def build_user_prompt(bundle: VerifierInput) -> str:
    """Assemble the user message from the bundle.

    The structure mirrors the verifier-bundle layout from FT-022: feature
    spec → produced artifact → relevant TCs → relevant ADRs. The model
    sees these in a fixed order so the prompt is deterministic.
    """
    tcs = "\n\n".join(_render_tc(t) for t in bundle.relevant_tcs) or "(none provided)"
    adrs = "\n\n".join(_render_adr(a) for a in bundle.relevant_adrs) or "(none provided)"
    return _USER_TEMPLATE.format(
        feature_id=bundle.feature_id,
        feature_spec=bundle.feature_spec,
        produced_iri=bundle.produced_artifact_iri or "(unset)",
        produced=bundle.produced_artifact,
        tcs=tcs,
        adrs=adrs,
        bundle_hash=bundle.bundle_hash,
    )


def build_retry_prompt(schema_error: str) -> str:
    """User-side nudge appended to the next call when the first response failed validation."""
    return (
        "Your previous response did not satisfy the VerificationVerdict schema. "
        f"Validation error:\n\n{schema_error}\n\n"
        "Return a single corrected JSON object matching the schema exactly. "
        "Required keys: verdict, rationale (≥ 20 chars), violates (≥ 1 when "
        "verdict ≠ approved). When verdict == amendment-required, include a "
        "non-empty amendment_guidance string. Cite TC/ADR identifiers from "
        "the bundle in violates."
    )


_USER_TEMPLATE = """\
Feature under verification: {feature_id}

## Feature specification

{feature_spec}

## Produced artifact ({produced_iri})

{produced}

## Test criteria

{tcs}

## Cross-cutting ADRs

{adrs}

## Audit

Bundle SHA-256 (action input): {bundle_hash}

Now produce the VerificationVerdict JSON object.
"""


def _render_tc(tc) -> str:
    head = f"### {tc.id}"
    if tc.type:
        head += f" ({tc.type})"
    body = tc.body.strip() or "(no body provided)"
    return f"{head}\n\n{body}"


def _render_adr(adr) -> str:
    head = f"### {adr.id}"
    if adr.scope:
        head += f" — scope: {adr.scope}"
    body = adr.body.strip() or "(no body provided)"
    return f"{head}\n\n{body}"
