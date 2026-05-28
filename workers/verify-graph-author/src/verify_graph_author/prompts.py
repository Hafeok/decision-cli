"""Builds the verify-graph-author system prompt plus per-bundle user/retry messages."""

from __future__ import annotations

import json

from .bundle import (
    DefectFeedbackRecord,
    EnrichmentFieldsRecord,
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
  - **The user message includes a "Bundle ground truth" section (ADR-066
    / FT-102) that defines the closed universe of dec commands,
    namespaces, binaries, and writable paths your proposal may
    reference. Any out-of-bundle reference (a command not in
    `cli_surface`, a namespace not in `ontology_vocabulary`, a binary
    not in `env_capabilities.binaries_on_path`) is rejected by the
    dispatch-time validator before persistence. In particular: use the
    EXACT `dec:` namespace string from `ontology_vocabulary.namespace`;
    do not substitute alternate forms.**
  - **If the bundle's `defect_feedback` array is non-empty (FT-107),**
    an existing covering graph has been observed to fail at runtime.
    You MUST NOT return `kind = match` in that case — read each entry's
    `evidence` field and propose a `new` graph that addresses the
    underlying problem. Cite each addressed feedback IRI in your
    `addressed_feedback_iris` field on the `new` proposal so the
    orchestrator can transition them to `addressed`.

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

==========================================================
SHELL VS SPARQL — THE MOST COMMON HALLUCINATION TO AVOID
==========================================================

When a TC asks you to verify that an artifact exists in the orchestration
store (a Role, a Capability, a feature, a TC, an ADR, an event-class
seed, etc.), the WRONG pattern is to write a shell pipeline like
`dec role list | grep <id>` or `dec catalog list | grep ...`. There is
no `dec role list`, no `dec catalog list`, no `dec metrics`, no
`dec sparql` subcommand outside the hidden test-helper. The
`cli_surface.dec_subcommands` block in the bundle lists every dec
command that actually exists — anything outside that list will be
rejected by the validator.

The RIGHT pattern is a `sparql-assertion` step against the on-disk
orchestration store. The store is at `.dec/store/orchestration.nq`
after `dec init` runs (see the EX-INIT-DOCTOR exemplar for the
init-first scaffold). Here's the canonical shape:

  {
    "step_type": "sparql-assertion",
    "fields": {
      "target": ".dec/store/orchestration.nq",
      "query": "PREFIX dec: <https://decision-cli.dev/ns#> SELECT ?role WHERE { GRAPH ?g { ?role a dec:Role ; dec:roleId \"verifier\" . } }",
      "expect-rows": 1
    },
    "provides_evidence_for": ["TC-027"]
  }

==========================================================
SPARQL — ALWAYS WRAP WITH `GRAPH ?g { ... }`
==========================================================

The `.dec/store/orchestration.nq` file uses RDF NAMED GRAPHS for
stream scoping — every artifact is in a named graph
(`https://decision-cli.dev/ns/streams/<stream>`), NOT in the default
graph. A SPARQL pattern that doesn't explicitly mention a graph
queries only the default graph, which is empty in this store.
That's why a query like

  SELECT ?role WHERE { ?role a dec:Role . }

returns 0 rows against a perfectly-populated store. You MUST wrap
your triple patterns with `GRAPH ?g { … }`:

  SELECT ?role WHERE { GRAPH ?g { ?role a dec:Role . } }

The same wrapping applies when asserting against `.product/graph/index.ttl`
— that file is also nq-shaped, with feature / TC / ADR artifacts in
their own named graphs. When in doubt, wrap.

Substitute the predicate, value, and expected row-count for what the
TC actually claims. Use the EXACT namespace string from
`ontology_vocabulary.namespace` in the `PREFIX dec:` declaration —
not `<https://decision-cli.dev/ns/>` (trailing slash) and not
`<https://decision-cli.dev/oxi-events/ns#>` (oxi-events is a crate
name, not a namespace — the namespace is the same dec namespace).

If the artifact you need to verify is in product-cli's graph
(feature, ADR, TC), use the same pattern but point `target` at
`.product/graph/index.ttl` instead.

==========================================================
ASK VS SELECT — `expect-rows` REQUIRES SELECT
==========================================================

A `sparql-assertion` step ASSERTS A ROW COUNT. The runner takes the
declared `expect-rows: N` and compares it to the number of solution
bindings the query returns. That comparison only works for SELECT
queries — those are the SPARQL form that produces rows.

  - `SELECT ?x WHERE { … }` → returns a sequence of rows. `expect-rows`
    is the count you assert about that sequence. Use this for every
    "the store should contain N artifacts matching …" assertion.
  - `ASK WHERE { … }` → returns ONE boolean (true / false). It does
    NOT produce rows. Pairing `ASK` with `expect-rows: 1` is a
    semantic mismatch — the runner will record 0 rows (because there
    is no row-shape projection at all) and the step will fail even
    when the underlying pattern would have matched. NEVER write
    `ASK WHERE …` inside a sparql-assertion step.
  - `CONSTRUCT` / `DESCRIBE` → also wrong shape; produce graphs, not
    rows. Don't use them in sparql-assertion steps either.

If the TC's success criterion is "the artifact exists" (and you don't
care about its other properties), write the SELECT form with the
single variable you want to count, like:

  "query": "PREFIX dec: <https://decision-cli.dev/ns#> SELECT ?role WHERE { GRAPH ?g { ?role a dec:Role ; dec:roleId \"verifier\" . } }",
  "expect-rows": 1

— not the ASK form. Read `expect-rows` aloud: "the SELECT returns
this many rows." If that sentence doesn't apply to your query, the
query is the wrong shape.

If the TC asserts "at least N", set `expect-rows: N` and pick a
SELECT whose binding count you expect to equal N. There is no
greater-than-or-equal operator — design the query so the row count
is exact.
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
    defect_block = _render_defect_feedback(bundle.defect_feedback)
    enrichment_block = _render_enrichment(bundle.enrichment)
    retry_block = _render_retry_warnings(bundle.enrichment)
    return _USER_TEMPLATE.format(
        retry_warnings=retry_block,
        feature_id=bundle.feature_id,
        feature_spec=bundle.feature_spec,
        tcs=tcs,
        env=_render_env(bundle.target_environment),
        vocabulary=vocabulary,
        candidates=candidates,
        defect_feedback=defect_block,
        enrichment=enrichment_block,
        bundle_hash=bundle.bundle_hash,
    )


def _render_retry_warnings(enrichment) -> str:
    """Surface any `RETRY:`-prefixed entries in
    `enrichment.bundle_metadata.warnings` at the very top of the user
    prompt so the model can't miss them. FT-110 worker-quality
    follow-up: the orchestrator pushes the previous-attempt
    validator-error into warnings on retry; rendering it prominently
    is the difference between the model correcting and the model
    re-emitting the same hallucination."""
    retries = [w for w in enrichment.bundle_metadata.warnings if w.startswith("RETRY:")]
    if not retries:
        return ""
    lines = [
        "## ⚠ Previous-attempt validator violations — READ THIS FIRST",
        "",
        "Your previous response was rejected by the dispatch-time validator. The errors are below.",
        "Re-author the proposal, addressing every cited violation. Out-of-bundle references",
        "(unknown dec subcommands, unknown SPARQL namespaces, unknown binaries) CANNOT be persisted",
        "and will be rejected again. Use only commands listed under `cli_surface.dec_subcommands`,",
        "namespaces listed under `ontology_vocabulary.namespaces`, and binaries listed under",
        "`env_capabilities.binaries_on_path`.",
        "",
    ]
    for w in retries:
        lines.append(f"- {w}")
    lines.append("")
    return "\n".join(lines)


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
{retry_warnings}## Goal

Propose a verification graph that produces evidence for each of the
following TCs in the given environment.

## Feature: {feature_id}

{feature_spec}

### Test criteria

{tcs}

## Target environment

{env}

## Bundle ground truth (ADR-066 / FT-102)

This section defines the **closed universe of values** your proposal
may reference. Anything outside these lists is rejected by the
dispatch-time validator before persistence. Treat each list as
authoritative; do not invent commands, namespaces, or binaries that
aren't here.

{enrichment}

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

## Defect feedback (FT-107)

{defect_feedback}

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


def _render_enrichment(enrichment: EnrichmentFieldsRecord) -> str:
    """Render the FT-102 bundle-completeness fields as authoritative ground
    truth. Each subsection is a closed universe of values the worker may
    reference; out-of-bundle references are rejected at dispatch time."""
    lines: list[str] = []

    cli = enrichment.cli_surface
    lines.append("### Available `dec` commands (cli_surface)")
    if cli.dec_subcommands:
        lines.append(
            "Use ONLY the following commands as the head of a `shell-command` step. Do "
            "NOT invent flags or subcommands beyond what's listed."
        )
        for cmd in cli.dec_subcommands:
            lines.append(f"- `{cmd}`")
    else:
        lines.append("(empty — no dec commands are catalog-registered for this version)")
    lines.append("")
    if cli.init_templates:
        lines.append("### Valid `dec init --template` values (cli_surface.init_templates)")
        lines.append(
            "When step 0 of an init-first graph uses `dec init --template <name>`, the "
            "`<name>` MUST be one of these — anything else exits with code 1 before any "
            "evidence step runs and the graph's verdict is amendment-required. There is "
            "no `decision-cli-development` template; the only template currently shipped "
            "with this `dec` is the one listed below."
        )
        for tpl in cli.init_templates:
            lines.append(f"- `{tpl}`")
        lines.append("")

    ont = enrichment.ontology_vocabulary
    lines.append("### Ontology vocabulary (ontology_vocabulary)")
    if ont.namespace:
        lines.append(
            f"**Canonical dec namespace (USE THIS EXACT STRING in `sparql-assertion` "
            f"PREFIX declarations):**"
        )
        lines.append("```")
        lines.append(f"PREFIX {ont.prefix or 'dec'}: <{ont.namespace}>")
        lines.append("```")
        lines.append(
            "Do NOT substitute `https://decision-cli.dev/ns/` (trailing slash) or any "
            "other variant — the validator rejects them as out-of-bundle."
        )
    else:
        lines.append("(no active OntologyDescription in the catalog)")
    if ont.namespaces:
        lines.append("Allowed namespaces (anything outside this list is rejected):")
        for ns in ont.namespaces:
            lines.append(f"- `{ns}`")
    if ont.classes:
        lines.append("Declared dec classes (local names):")
        lines.append("  " + ", ".join(ont.classes))
    lines.append("")

    sq = enrichment.store_query_surface
    lines.append("### Store query surface (store_query_surface)")
    if sq.kind or sq.query_command:
        lines.append(f"- kind: `{sq.kind or '(unset)'}`")
        if sq.query_command:
            lines.append(f"- query_command: `{sq.query_command}`")
        if sq.endpoint:
            lines.append(f"- endpoint: `{sq.endpoint}`")
    else:
        lines.append("(default — use the env's local store)")
    lines.append("")

    env_caps = enrichment.env_capabilities
    lines.append("### Environment capabilities (env_capabilities)")
    if env_caps.binaries_on_path:
        lines.append(
            "Binaries you may use as the head of a `shell-command`. Anything else is "
            "rejected (e.g. `mkdir`, `touch`, `curl` are NOT free unless listed):"
        )
        lines.append("  " + ", ".join(f"`{b}`" for b in env_caps.binaries_on_path))
    else:
        lines.append("(no binaries declared — fall back to the dec subcommands above)")
    if env_caps.writable_paths:
        lines.append("Writable path prefixes (steps must stay within these):")
        for p in env_caps.writable_paths:
            lines.append(f"- `{p}`")
    if env_caps.allowed_hosts:
        lines.append("HTTP hosts allowed in `http-request` steps:")
        for h in env_caps.allowed_hosts:
            lines.append(f"- `{h}`")
    if env_caps.environment_variables:
        lines.append("Env-vars you may reference in `capture` steps:")
        lines.append("  " + ", ".join(f"`${v}`" for v in env_caps.environment_variables))
    lines.append("")

    if enrichment.exemplar_graphs:
        lines.append("### Exemplar graphs (exemplar_graphs)")
        lines.append(
            "Curated, known-good `VerificationGraph` patterns for this env's safety "
            "class. Use them as templates."
        )
        for ex in enrichment.exemplar_graphs:
            head = f"- {ex.id}"
            if ex.pattern_name:
                head += f" — `{ex.pattern_name}`"
            lines.append(head)
            if ex.rationale:
                lines.append(f"  · {ex.rationale}")

    md = enrichment.bundle_metadata
    if md.warnings:
        lines.append("")
        lines.append("### Bundle warnings (bundle_metadata.warnings)")
        for w in md.warnings:
            lines.append(f"- {w}")

    return "\n".join(lines).rstrip()


def _render_defect_feedback(records: list[DefectFeedbackRecord]) -> str:
    if not records:
        return (
            "(no defect feedback for this (feature, env) pair — there is no runtime "
            "evidence forcing a re-author. Normal matcher / vocabulary rules apply.)"
        )
    lines = [
        "An existing covering graph produced these failures at runtime. You MUST NOT "
        "return `kind = match` against that graph — read each entry's `evidence` field "
        "and propose a `new` graph that addresses the underlying problem. The most "
        "common cause is an env-type mismatch: e.g. an `ephemeral-tempdir` env paired "
        "with shell commands that need the repository to be present (`cargo build`, "
        "`grep crates/...`). In that case, propose a `new` graph whose env is "
        "`repo-path` (or whose steps populate the tempdir first).",
        "",
        "**Cite the `feedback_iri` of every entry you actually addressed in your "
        "`addressed_feedback_iris` field on the `new` proposal — the orchestrator uses "
        "that list to mark the feedback as resolved.**",
        "",
    ]
    for rec in records:
        lines.append(f"### {rec.feedback_iri}")
        if rec.graph_id:
            lines.append(f"- failing graph: {rec.graph_id}")
        if rec.source_tc:
            lines.append(f"- source TC: {rec.source_tc}")
        lines.append(f"- severity: {rec.severity}")
        lines.append(f"- evidence: {rec.evidence.strip() or '(empty)'}")
        lines.append("")
    return "\n".join(lines).rstrip()


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
