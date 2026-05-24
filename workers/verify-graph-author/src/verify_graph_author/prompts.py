"""Builds the verify-graph-author system prompt plus per-bundle user/retry messages."""

from __future__ import annotations

import json

from .bundle import (
    EnvRecord,
    ExistingGraphRecord,
    StepKindRecord,
    TcRecord,
    VerifyGraphAuthorInput,
)

SYSTEM_PROMPT = """\
You are the verify-graph-author role in decision-cli's Decision-Driven
Design pipeline.

Your single responsibility: given a feature_spec, its test criteria
(TCs), exactly one target verification environment, the catalog of
existing graphs that touch any of the feature's TCs in that env, and a
controlled step-kind vocabulary, propose a verification graph that
produces evidence for each of the listed TCs in the given environment.

You return exactly one GraphProposal, with one of three kinds:

  - "match": one of the candidate existing graphs already covers all of
    the feature's TCs in this env. Set `match.graph_id` to its id and
    write a rationale that names which TCs the existing graph covers and
    how.
  - "new": no existing graph covers the feature's TCs adequately, so
    propose a fresh ordered list of steps in the target environment. You
    may only use step kinds from the supplied vocabulary. Every step
    must include `provides_evidence_for` (a list of TC ids, possibly
    empty for explicit setup or capture steps). Together the steps must
    cover every TC in the bundle. You may borrow patterns from the
    candidate graphs — name them in your rationale.
  - "gap": you cannot, in good faith, propose a graph that covers the
    feature's TCs with the available vocabulary and the environment's
    `allowed_ops`. Set `gap.uncovered_tcs` to the TC ids you cannot
    cover and `gap.reason` to a brief explanation. Prefer `gap` over
    inventing synthetic or invalid steps.

Hard constraints (any violation invalidates the proposal):

  - Only the step kinds listed under "Step vocabulary" are valid.
  - Steps that need operations not in the target environment's
    `allowed_ops` MUST NOT appear in a `new` proposal — return `gap`
    instead.
  - **Every step's `fields` object MUST contain every key listed as
    REQUIRED under that step kind's vocabulary entry.** A step's `fields`
    is NOT freeform — it is a typed payload. Missing a REQUIRED key
    (e.g. `command` on `shell-command`, `query`+`target` on
    `sparql-assertion`, `path` on `file-assertion`) invalidates the
    whole proposal. Use exactly the field names shown in the vocabulary
    (kebab-case: `expect-exit-code` not `expectExitCode`).
  - You do not have tool access. You judge from the bundle text alone.
  - You do not modify, augment, or invent TC identifiers or env
    identifiers — use the exact ids from the bundle.
  - `bundle_hash` in your output MUST be the exact `bundle_hash` from
    the input bundle. Do not invent, redact, or modify it.

Output a single JSON object that matches the GraphProposal schema
exactly: top-level keys `kind`, `bundle_hash`, and exactly one of
`match`, `new`, or `gap` populated.

Example of a well-formed `new` proposal step (note `fields` populated
with the REQUIRED keys for the step's kind):

  {
    "step_type": "shell-command",
    "fields": {
      "command": "dec doctor --format json",
      "expect-exit-code": 0,
      "capture-output": true
    },
    "provides_evidence_for": ["TC-046"]
  }
"""


def build_user_prompt(bundle: VerifyGraphAuthorInput) -> str:
    """Assemble the user message from the bundle.

    The structure mirrors ADR-030 §"Bundle contract": goal → feature →
    environment → vocabulary → candidates → audit. Order is fixed so the
    prompt is deterministic across runs.
    """
    tcs = "\n\n".join(_render_tc(t) for t in bundle.relevant_tcs) or "(no TCs supplied)"
    vocabulary = "\n\n".join(_render_kind(k) for k in bundle.step_vocabulary) or "(no kinds)"
    candidates = (
        "\n\n".join(_render_candidate(g) for g in bundle.candidate_graphs)
        or "(no candidate graphs in this environment)"
    )
    return _USER_TEMPLATE.format(
        feature_id=bundle.feature_id,
        feature_spec=bundle.feature_spec,
        tcs=tcs,
        env=_render_env(bundle.target_environment),
        vocabulary=vocabulary,
        candidates=candidates,
        bundle_hash=bundle.bundle_hash,
    )


def build_retry_prompt(schema_error: str) -> str:
    """User-side nudge appended to the next call when the first response failed validation."""
    return (
        "Your previous response did not satisfy the GraphProposal schema. "
        f"Validation error:\n\n{schema_error}\n\n"
        "Return a single corrected JSON object matching the schema exactly. "
        "Required top-level keys: kind ('match' | 'new' | 'gap'), bundle_hash "
        "(echo the exact value from the bundle), and exactly one of `match`, "
        "`new`, or `gap` populated to match `kind`. Use only the supplied step "
        "kinds and respect the environment's allowed_ops. If you cannot honestly "
        "cover the feature's TCs, return a `gap`."
    )


_USER_TEMPLATE = """\
## Goal

Propose a verification graph that produces evidence for each of the
following TCs in the given environment.

## Feature: {feature_id}

{feature_spec}

### Test criteria

{tcs}

## Target environment

{env}

## Step vocabulary

You may only use the listed step kinds. If your strategy needs
operations not in the target environment's `allowed_ops`, return a
`gap`.

{vocabulary}

## Candidate graphs

If one of these graphs already adequately covers the feature's TCs,
return a `match` with its id and your rationale. Otherwise return a
`new` graph. You may borrow patterns from candidates — name them in
your rationale.

{candidates}

## Audit

bundle_hash (echo this verbatim in your proposal): {bundle_hash}

Now produce the GraphProposal JSON object.
"""


def _render_tc(tc: TcRecord) -> str:
    head = f"### {tc.id}"
    if tc.title:
        head += f" — {tc.title}"
    body = tc.body.strip() or "(no body provided)"
    return f"{head}\n\n{body}"


def _render_env(env: EnvRecord) -> str:
    parts = [
        f"- id: {env.id}",
        f"- env_type: {env.env_type}",
    ]
    if env.safety_class:
        parts.append(f"- safety_class: {env.safety_class}")
    if env.allowed_ops:
        parts.append(f"- allowed_ops: {', '.join(env.allowed_ops)}")
    else:
        parts.append("- allowed_ops: (none)")
    if env.endpoint:
        parts.append(f"- endpoint: {env.endpoint}")
    return "\n".join(parts)


def _render_kind(kind: StepKindRecord) -> str:
    head = f"### {kind.kind}"
    parts = [head]
    if kind.description:
        parts.append(kind.description.strip())
    parts.append(
        "- required_ops: "
        + (", ".join(kind.required_ops) if kind.required_ops else "(none)")
    )
    parts.extend(_render_fields_block(kind.fields_schema))
    return "\n".join(parts)


def _render_fields_block(schema: dict) -> list[str]:
    """Render `fields_schema` as an LLM-friendly bulleted list instead of
    a raw JSON dump.

    Models (Qwen3-Coder in particular) tended to skim the JSON dump and
    populate `fields: {}` even when required keys were declared. The
    bulleted form calls out REQUIRED vs optional explicitly.
    """
    properties = (schema.get("properties") or {}) if isinstance(schema, dict) else {}
    required = set(schema.get("required") or []) if isinstance(schema, dict) else set()
    if not properties:
        return ["- fields: (no payload — `fields: {}`)"]
    lines = ["- fields (populate `step.fields` with these keys):"]
    # Stable order: required first (in declaration order), then optional.
    req_keys = [k for k in properties if k in required]
    opt_keys = [k for k in properties if k not in required]
    for key in req_keys + opt_keys:
        spec = properties.get(key, {}) if isinstance(properties, dict) else {}
        marker = "REQUIRED" if key in required else "optional"
        type_label = spec.get("type", "string") if isinstance(spec, dict) else "string"
        desc = spec.get("description", "") if isinstance(spec, dict) else ""
        lines.append(f"  - `{key}` ({type_label}, {marker}): {desc}".rstrip(": "))
    # Final JSON dump kept as a fallback for models that prefer reading it.
    lines.append("- fields_schema (raw): " + json.dumps(schema, sort_keys=True))
    return lines


def _render_candidate(graph: ExistingGraphRecord) -> str:
    head = f"### {graph.id}"
    parts = [head]
    if graph.verifies:
        parts.append(f"- verifies: {graph.verifies}")
    if graph.covers:
        parts.append(f"- covers: {', '.join(graph.covers)}")
    else:
        parts.append("- covers: (none)")
    if graph.step_summaries:
        parts.append("- steps:")
        for i, step in enumerate(graph.step_summaries, start=1):
            ev = ", ".join(step.provides_evidence_for) or "(none)"
            summary = step.summary.strip() or "(no summary)"
            parts.append(f"  {i}. [{step.step_type}] {summary} — evidence: {ev}")
    return "\n".join(parts)
