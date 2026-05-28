"""Parses an incoming verify-graph-author dispatch bundle into a VerifyGraphAuthorInput struct."""

from __future__ import annotations

from typing import Any

from pydantic import BaseModel, Field


class TcRecord(BaseModel):
    """One test criterion the proposed graph must produce evidence for."""

    id: str = Field(..., description="Test criterion identifier, e.g. 'TC-013'.")
    title: str = Field(default="", description="Short TC title for the prompt.")
    body: str = Field(default="", description="Markdown body of the TC.")
    runner: str = Field(
        default="",
        description=(
            "Runner name from TC frontmatter (e.g. 'bash', 'cargo-test', 'pytest'). "
            "When present, the graph step for this TC should invoke this runner "
            "against `runner_args` — do not synthesize an alternative command."
        ),
    )
    runner_args: str = Field(
        default="",
        description=(
            "Args the runner takes (e.g. 'tests/scripts/tc-007-unauthorized-goal.sh'). "
            "Pair with `runner` to build the canonical shell-command for the step."
        ),
    )
    runner_timeout: str = Field(
        default="",
        description="Suggested timeout (e.g. '30'). Informational; the step runner uses its own per-kind defaults.",
    )


class EnvRecord(BaseModel):
    """The target verification environment for this proposal."""

    id: str = Field(..., description="Environment identifier, e.g. 'ENV-1'.")
    env_type: str = Field(
        ...,
        description="Environment kind label (e.g. 'ephemeral-tempdir', 'http-endpoint').",
    )
    safety_class: str = Field(
        default="",
        description="Safety classification (e.g. 'read-only', 'mutating').",
    )
    allowed_ops: list[str] = Field(
        default_factory=list,
        description="Operations the environment permits. Steps requiring ops "
        "not in this list must NOT appear in a New proposal.",
    )
    endpoint: str | None = Field(
        default=None,
        description="Optional endpoint URL when relevant (e.g. for http-* envs).",
    )


class StepSummary(BaseModel):
    """A condensed view of one step in an existing graph (for the prompt)."""

    step_type: str = Field(..., description="Step kind (e.g. 'shell-command').")
    summary: str = Field(default="", description="Short description of what the step does.")
    provides_evidence_for: list[str] = Field(
        default_factory=list,
        description="TC ids this step provides evidence for in the existing graph.",
    )


class ExistingGraphRecord(BaseModel):
    """One existing verification graph the worker may match against."""

    id: str = Field(..., description="Existing graph identifier, e.g. 'VG-007'.")
    verifies: str = Field(
        default="",
        description="Feature this graph verifies (may be the same as the bundle's feature).",
    )
    covers: list[str] = Field(
        default_factory=list,
        description="TC ids this existing graph currently covers in the target env.",
    )
    step_summaries: list[StepSummary] = Field(
        default_factory=list,
        description="Ordered, condensed list of the graph's steps.",
    )


class CliCommandRecord(BaseModel):
    """One structured `dec:CapabilityReference` row in the bundle."""

    command: str = Field(..., description="Canonical command string, e.g. 'dec verify graph new'.")
    capability_version: str = Field(default="", description="Capability version, e.g. '0.3.0'.")
    source_cr: str = Field(default="", description="Source CR id (e.g. 'CR-INIT').")


class CliSurfaceRecord(BaseModel):
    """ADR-066 / FT-102 — every dec subcommand the worker may invoke."""

    commands: list[CliCommandRecord] = Field(default_factory=list)
    dec_subcommands: list[str] = Field(
        default_factory=list,
        description="Flat list of `dec <subcommand>` strings for cheap lookup.",
    )
    capability_version: str = Field(default="", description="dec version this surface was resolved against.")
    init_templates: list[str] = Field(
        default_factory=list,
        description=(
            "Valid value-stream template names for `dec init --template <name>`. "
            "When authoring an init step, pick from this list — hallucinated names "
            "fail at exit 1 and produce no useful evidence. If the list is empty, "
            "fall back to `dec init --from <path>` against a known-good .ttl."
        ),
    )


class OntologyVocabularyRecord(BaseModel):
    """ADR-066 / FT-102 — the canonical dec namespace + class vocabulary."""

    namespace: str = Field(
        default="",
        description=(
            "Canonical dec namespace IRI (e.g. 'https://decision-cli.dev/ns#'). USE THIS "
            "EXACT STRING as the `dec:` PREFIX value in every `sparql-assertion` step; any "
            "deviation (trailing slash vs hash, alternate origins) is rejected by the "
            "dispatch-time validator."
        ),
    )
    prefix: str = Field(default="dec", description="Prefix alias for the namespace.")
    namespaces: list[str] = Field(
        default_factory=list,
        description="All namespaces the worker may reference; anything outside this list is rejected.",
    )
    classes: list[str] = Field(default_factory=list, description="Class local names from the active OntologyDescription.")
    source_od: str = Field(default="", description="Source OD id (e.g. 'OD-001').")


class StoreQuerySurfaceRecord(BaseModel):
    """How to address the orchestration store from inside a step."""

    kind: str = Field(default="", description="Surface kind: 'local-oxigraph' or 'remote-http'.")
    query_command: str = Field(default="", description="Literal command or template to query the store.")
    endpoint: str | None = Field(default=None, description="Optional endpoint URL.")


class EnvCapabilitiesRecord(BaseModel):
    """Concrete env capabilities the worker may rely on at runtime."""

    binaries_on_path: list[str] = Field(
        default_factory=list,
        description=(
            "Binaries the worker may use as the head of a `shell-command`. Anything not "
            "in this list is rejected by the dispatch validator."
        ),
    )
    writable_paths: list[str] = Field(
        default_factory=list,
        description="File-path prefixes the worker may write to (steps must stay within these).",
    )
    allowed_hosts: list[str] = Field(default_factory=list, description="HTTP hosts allowed in `http-request`.")
    environment_variables: list[str] = Field(
        default_factory=list,
        description="Env-var names the worker may reference in `capture`.",
    )
    pre_seeded_artifacts: list[str] = Field(default_factory=list, description="Artifacts visible in the env.")


class ExemplarRecord(BaseModel):
    """One curated exemplar VerificationGraph the worker may pattern-match against."""

    id: str = Field(..., description="EX-NNN id.")
    exemplar_of: str = Field(default="", description="IRI of the underlying VG.")
    pattern_name: str = Field(default="", description="Short pattern slug.")
    rationale: str = Field(default="", description="Long-form rationale.")
    safety_class: str = Field(default="", description="Safety class this exemplar applies to.")


class CatalogHashEntry(BaseModel):
    id: str = Field(..., description="Short artifact id (CR-NNN | OD-NNN | EX-NNN).")
    content_hash: str = Field(default="", description="SHA-256 hex of the canonical serialisation.")


class BundleMetadataRecord(BaseModel):
    catalog_hashes: list[CatalogHashEntry] = Field(default_factory=list)
    warnings: list[str] = Field(default_factory=list)


class EnrichmentFieldsRecord(BaseModel):
    """ADR-066 / FT-102 — the five bundle-completeness fields the
    verify-graph-author worker reads as ground truth for its proposal.

    The Rust bundle assembler populates these by SPARQL-querying the
    orchestration store's catalog graph. The dispatch-time validator
    rejects any proposal that references commands / namespaces / paths /
    binaries outside the bundle, so the worker must treat each field as
    the closed universe of values it may reference."""

    cli_surface: CliSurfaceRecord = Field(default_factory=CliSurfaceRecord)
    ontology_vocabulary: OntologyVocabularyRecord = Field(default_factory=OntologyVocabularyRecord)
    store_query_surface: StoreQuerySurfaceRecord = Field(default_factory=StoreQuerySurfaceRecord)
    env_capabilities: EnvCapabilitiesRecord = Field(default_factory=EnvCapabilitiesRecord)
    exemplar_graphs: list[ExemplarRecord] = Field(default_factory=list)
    bundle_metadata: BundleMetadataRecord = Field(default_factory=BundleMetadataRecord)


class DefectFeedbackRecord(BaseModel):
    """FT-107 — one defect-feedback artifact the bundle assembler picked up
    for the (feature, env) pair.

    Non-empty `defect_feedback` on the input bundle is the orchestrator's
    explicit signal that an existing covering graph produced failures at
    runtime and the worker should re-author rather than return Match.
    """

    feedback_iri: str = Field(..., description="Stable urn:dec:feedback:<uuid>.")
    class_: str = Field(
        default="defect",
        alias="class",
        description="Controlled vocabulary tag; always 'defect' for entries this bundle carries.",
    )
    severity: str = Field(default="error", description="'error' | 'warning' | 'info'.")
    evidence: str = Field(
        default="",
        description="Free-form evidence excerpt the runner wrote at emission time — read this to understand what failed.",
    )
    source_tc: str = Field(
        default="",
        description="TC IRI the feedback's dec:sourceArtifact points at.",
    )
    graph_id: str = Field(
        default="",
        description="Short id of the graph whose failing step covers source_tc (e.g. 'VG-007').",
    )

    model_config = {"populate_by_name": True}


class StepKindRecord(BaseModel):
    """One available step kind in the worker's vocabulary."""

    kind: str = Field(..., description="Step kind name (one of the six seed kinds).")
    required_ops: list[str] = Field(
        default_factory=list,
        description="Operations this kind needs at execution time; must be a "
        "subset of `target_environment.allowed_ops` for a step of this kind "
        "to be valid in the target environment.",
    )
    fields_schema: dict[str, Any] = Field(
        default_factory=dict,
        description="JSON-schema-shaped contract for the step's `fields` payload.",
    )
    description: str = Field(default="", description="Human-readable purpose of the kind.")


class VerifyGraphAuthorInput(BaseModel):
    """Inputs the harness hands the verify-graph-author for a single proposal.

    Matches the bundle FT-049's assembler builds. The worker treats the
    bundle as a complete, self-contained context: it does NOT fetch
    additional files, query the graph, or make follow-up calls
    (ADR-008 / ADR-030).
    """

    feature_id: str = Field(..., description="Feature being verified (e.g. 'FT-007').")
    feature_spec: str = Field(..., description="Full feature_spec markdown body.")
    relevant_tcs: list[TcRecord] = Field(
        default_factory=list,
        description="Test criteria the proposed graph must cover.",
    )
    target_environment: EnvRecord = Field(
        ...,
        description="Exactly one target environment per call (ADR-030).",
    )
    candidate_graphs: list[ExistingGraphRecord] = Field(
        default_factory=list,
        description="Existing graphs that touch any of the feature's TCs in this env.",
    )
    step_vocabulary: list[StepKindRecord] = Field(
        default_factory=list,
        description="Step kinds the worker is allowed to propose.",
    )
    bundle_hash: str = Field(
        ...,
        min_length=8,
        description="SHA-256 hex of the canonical bundle (echoed in the proposal).",
    )
    endpoint: str = Field(
        default="anthropic",
        description="Endpoint discriminator resolved by the dispatcher "
        "(FT-061): 'anthropic' or 'scaleway'.",
    )
    model_id: str = Field(
        ...,
        description="Provider-specific model identifier pinned by the "
        "dispatcher per FT-061. Workers do not hardcode this; it arrives "
        "in the dispatch payload.",
    )
    parameters: dict = Field(
        default_factory=dict,
        description="Additional capability-resolved parameters forwarded "
        "to the router untouched.",
    )
    max_tokens: int = Field(default=4096, ge=256, le=64_000)
    enrichment: EnrichmentFieldsRecord = Field(
        default_factory=EnrichmentFieldsRecord,
        description=(
            "ADR-066 / FT-102 — the five catalog-derived fields that define the closed "
            "universe of commands / namespaces / paths / binaries this proposal may "
            "reference. The dispatch-time validator rejects any out-of-bundle reference."
        ),
    )
    defect_feedback: list[DefectFeedbackRecord] = Field(
        default_factory=list,
        description="FT-107: runtime defect feedback the existing covering graph(s) "
        "produced for this (feature, env) pair. Non-empty means: re-author from "
        "this evidence; do NOT return Match.",
    )

    # `model_id` collides with pydantic v2's reserved `model_` prefix.
    model_config = {"protected_namespaces": ()}

    @property
    def tc_ids(self) -> list[str]:
        """Identifiers of every TC the proposal must address."""
        return [t.id for t in self.relevant_tcs]
