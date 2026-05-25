# Working session: worker-distribution slice 1

Authoring format: option-2 (one file per working session, typed sections per
artifact, `@<predicate> <ref>` for edges). Predicate names and ID conventions
remain proposals — adjust to whatever product-cli's catalog actually uses.

Revision note: updated to fold in GitHub Actions release flow (keyless cosign
via GitHub OIDC from slice 1), the `pipeline-cli workers run` subcommand and
env-based secret handling for the manual runtime, the explicit submission
endpoint on pipeline-cli, the LiteLLM proxy deployment as the LLM-call
substrate (originally deferred to slice 2, pulled forward), and ADRs covering
secrets and the LiteLLM choice. Provider keys now live in LiteLLM's config
exclusively; workers hold only a virtual key.

---

## Brief brief:worker-distribution-slice-1

title: Build worker distribution slice 1 — OCI packaging, signing, catalog admission, and the GitHub Actions release flow

@references brief:pipeline-worker-slice-1      (sibling SDK Brief)
@references brief:dual-provenance-discipline   (dependency)
@references doc:impl-doc-§9                    (model catalog pattern this mirrors)
@references doc:entity-reference-policy

@decomposes_into feature:worker-image-artifact-type
@decomposes_into feature:worker-image-submission-type
@decomposes_into feature:oci-packaging-conventions
@decomposes_into feature:cosign-signing-flow
@decomposes_into feature:identity-verification-action
@decomposes_into feature:sbom-attachment
@decomposes_into feature:worker-curator-role
@decomposes_into feature:release-workflow
@decomposes_into feature:submission-endpoint
@decomposes_into feature:manual-runtime-stance
@decomposes_into feature:litellm-proxy-deployment

@excludes feature:worker-supervisor              (slice 4+)
@excludes feature:automated-conformance-replay   (slice 2+, depends on conformance corpus)
@excludes feature:vuln-scanning-gate             (slice 3+)
@excludes feature:autoscaling                    (slice 4+)
@excludes feature:dagger-runtime                 (deferred per adr:dagger-deferred)
@excludes feature:wasm-runtime                   (later, if a narrow pure-execution case emerges)
@excludes feature:secrets-manager-integration    (slice 2+ alternative; LiteLLM virtual keys cover slice 1)
@excludes feature:multi-tenant-litellm           (slice 3+; one LiteLLM deployment per tenant when multi-tenancy lands)
@excludes feature:multi-tenant-registry          (slice 3+)
@excludes feature:dynamic-binding-rebalancing    (slice 4+, meta-loop concern)

@acknowledges ack:slice-1-single-tenant
@acknowledges ack:manual-curator-decisions
@acknowledges ack:conformance-corpus-not-yet-exists
@acknowledges ack:env-var-secret-trust-model

premise:
  Workers currently install via uv install from a repo. There's no mechanism to
  package them, sign them, verify them, admit them to an eligible set, run them
  under any kind of policy, or handle their secrets in any structured way. As
  soon as there's more than one worker — or more than one worker version, or
  workers from more than one author — the absence of a registration mechanism
  becomes structural debt that affects every dispatch decision. The
  orchestration system can't reason about which workers are eligible for which
  capability tags without an artifact catalog parallel to the one it already
  has for models.

goal:
  Establish the worker registration discipline end-to-end, from producer-side
  CI to admission to startup, including the LLM-call substrate workers depend
  on. Slice 1 ships: OCI image format with declared label conventions, cosign
  keyless signing via GitHub OIDC (CI is in scope from day one, so the
  slice-1 simplification of local keys doesn't pay off), CycloneDX SBOM
  attached as OCI referrer, a GitHub Actions reusable workflow that worker
  repos call to release a new version, a submission endpoint on pipeline-cli
  that the workflow posts WorkerImageSubmissions to, a WorkerCurator role
  that admits or rejects submissions (manual review in slice 1; conformance
  corpus doesn't exist yet), a WorkerImage artifact type in the orchestration
  catalog, a LiteLLM proxy deployment that workers route every LLM call
  through (per adr:litellm-as-llm-proxy-slice-1), and a `pipeline-cli workers
  run` subcommand that starts a worker container locally with the LiteLLM
  virtual key and pipeline-cli bearer token injected as env vars from a
  local config file.

success_criteria:
  - A worker source repo's release workflow runs on tag push, builds the OCI
    image, signs it keyless via cosign + GitHub OIDC, attaches an SBOM, pushes
    to ghcr.io, and posts a WorkerImageSubmission to pipeline-cli.
  - The Submission enters the orchestration system; the WorkerCurator (human)
    can admit it (producing a WorkerImage with eligibility_status=qualified)
    or reject it with feedback.
  - A LiteLLM proxy is deployed (locally for slice 1) with at least one
    model group configured (Anthropic via Anthropic's API). LiteLLM holds the
    provider API key; workers do not.
  - `pipeline-cli workers run <worker-image-id>` pulls the qualified image and
    starts a container locally with environment variables sourced from a local
    config file: pipeline-cli bearer token, pipeline-cli endpoint URL,
    LiteLLM endpoint URL, LiteLLM virtual key.
  - Orchestration policy can bind a capability tag to the qualified
    WorkerImage; the next dispatch for that tag is delivered to the running
    process via the SDK's SSE/POST contract. The worker calls LiteLLM for its
    LLM completion; LiteLLM's logging callback posts call telemetry back to
    pipeline-cli's reconciliation endpoint.

---

## Feature feature:worker-image-artifact-type

title: Define the WorkerImage artifact type in the orchestration catalog

@motivated_by brief:worker-distribution-slice-1
@addresses_decision adr:worker-image-mirrors-model-catalog

The orchestration catalog gains a WorkerImage type parallel to the Model type
from impl doc §9. SHACL shape:

```
WorkerImage
├── id, name, version
├── registry_ref         — OCI reference with digest
├── capability_tags      — set of strings
├── compatible_roles     → Role[]
├── signed_by            — sigstore identity (Fulcio cert subject + issuer)
├── sbom_ref             — OCI referrer URI
├── conformance_audits   → ConformanceAudit[]
├── eligibility_status   — qualified | candidate | deprecated | pulled
├── provenance           — source repo URI, commit hash, GitHub Actions run URL
└── mechanical + motivational provenance per dual-provenance discipline
```

Motivational origin: at least one of `addresses Feedback` (image responds to
operational issue), `decomposes_from Brief` (image built to advance a slice),
or `originated_from DiscoveryFinding` (image built in response to a discovery
of an unmet capability). The first WorkerImage will be motivated by this Brief.

---

## Feature feature:worker-image-submission-type

title: Define WorkerImageSubmission as the initial-request artifact for admission

@motivated_by brief:worker-distribution-slice-1

When a worker author releases a new version, their CI produces a
WorkerImageSubmission artifact carrying: candidate registry_ref, claimed
capability_tags, claimed compatible_roles, sbom_ref, Fulcio signature identity,
and provenance fields (source repo URI, commit hash, GitHub Actions run URL).
The Submission enters the orchestration system through the intake at
feature:submission-endpoint — it's an initial-request artifact at the system
boundary, with no upstream motivational origin in the orchestration graph
itself (its origin lives in the producer's repo/CI).

The Submission is the bundle the WorkerCurator role's session consumes. The
Curator produces either a WorkerImage (admission) or a rejection Feedback
artifact.

---

## Feature feature:oci-packaging-conventions

title: Specify the OCI image conventions every worker must follow

@motivated_by brief:worker-distribution-slice-1
@addresses_decision adr:oci-format-over-wheels-and-custom
@addresses_decision adr:capability-tags-as-oci-labels

A worker OCI image MUST:
- Carry capability tags as OCI labels: `ddd.capability-tag.<tag>=true` per
  tag claimed. Machine-readable from the manifest without pulling the image.
- Pin the worker SDK version in a label: `ddd.sdk-version=<semver>`.
- Pin the wire-protocol version in a label: `ddd.wire-protocol=<semver>`.
- Declare its entrypoint as a long-running worker process that opens the
  SSE connection on start, reading pipeline-cli's endpoint and the bearer
  token from environment variables.
- Be multi-arch where reasonable (at least linux/amd64 and linux/arm64).
- Carry an OCI annotation pointing to the source repo and commit hash.

Slice 1 ships a base image (`pipeline-worker-base:<version>`) that worker
authors extend. The base image bakes in the SDK and the SSE/POST loop;
authors add their worker logic and metadata labels on top.

---

## Feature feature:cosign-signing-flow

title: Sign WorkerImages keyless via cosign and GitHub OIDC

@motivated_by brief:worker-distribution-slice-1
@addresses_decision adr:cosign-keyless-via-github-oidc-slice-1

Slice 1 ships keyless cosign signing using the ambient GitHub OIDC token in
the release workflow. No key material in repos. The signing identity is the
Fulcio-issued certificate's subject (the GitHub Actions workflow run identity).
The orchestration system keeps a trust list of permitted Fulcio identities
(by repo, by workflow path, by tag pattern); only signatures from listed
identities are valid.

The Rekor transparency log entry produced by cosign sign is referenced from
the Submission so the verifier can confirm both signature validity and log
inclusion.

Local key-based signing remains supported as a fallback for development
workflows that don't run through GitHub Actions, but is not the primary path.

---

## Feature feature:identity-verification-action

title: Cosign-verify action that runs as part of the admission process

@motivated_by brief:worker-distribution-slice-1

Action role: `identity-verifier`. Flavor: pure_execution + interpretation.

The action: runs `cosign verify` against the candidate image's signature using
the trust list (permitted Fulcio identities). The interpretation: paired
decision session that produces a signature-validity verdict artifact (`valid`,
`invalid-signature`, `untrusted-identity`, `image-not-found`,
`rekor-entry-missing`).

Verdict feeds into the WorkerCurator's bundle.

---

## Feature feature:sbom-attachment

title: Attach CycloneDX SBOM as an OCI referrer

@motivated_by brief:worker-distribution-slice-1
@addresses_decision adr:sbom-as-oci-referrer

Workers produce a CycloneDX SBOM (using syft or equivalent in the release
workflow) and attach it to the OCI image as a referrer per OCI v1.1. The
`sbom_ref` field on WorkerImage points to the referrer descriptor.

Slice 1 makes the SBOM available; it does not gate admission on vulnerability
scan results (slice 3 work). The Curator notes the SBOM presence and
references it in the WorkerImage but doesn't run a scanner.

---

## Feature feature:worker-curator-role

title: Define the WorkerCurator role that admits or rejects WorkerImageSubmissions

@motivated_by brief:worker-distribution-slice-1
@addresses_decision adr:manual-conformance-slice-1

A decision role in the orchestration system. Bundle:
- The WorkerImageSubmission (focal artifact)
- The identity-verification verdict
- The SBOM (referenced; not scanned in slice 1)
- The current orchestration policy (capacity, capability-tag coverage,
  preferred provenance constraints)
- Existing WorkerImages with overlapping capability tags (for comparison)

Output: either a WorkerImage with `eligibility_status=qualified` (admission)
or a Feedback artifact with `class=submission-rejected` and evidence
(rejection). The Submission's lifecycle state transitions accordingly.

Slice 1: the Curator is human-filled (Level 0 autonomy). The bundle assembly
and curated query helpers exist; a human reads the bundle and authors the
output. Slice 4+ work graduates the role per measurement evidence.

The role's motivational provenance discipline applies: every WorkerImage it
produces traces back through the Submission to the Submission's external
origin.

---

## Feature feature:release-workflow

title: GitHub Actions reusable workflow for releasing workers

@motivated_by brief:worker-distribution-slice-1
@addresses_decision adr:reusable-workflow-vs-per-repo

A single reusable workflow (`release-worker.yml`) hosted in pipeline-cli's
repo (or a dedicated workflows repo), called from each worker's
`.github/workflows/release.yml` on tag push. The reusable workflow handles
the entire producer-side release flow:

1. Checkout, set up build environment.
2. Read the worker's manifest (capability tags, compatible roles, SDK version,
   wire-protocol version, entrypoint).
3. Build the OCI image multi-arch via buildx, injecting labels per
   feature:oci-packaging-conventions.
4. Generate CycloneDX SBOM (syft).
5. Push image to ghcr.io with the version tag.
6. cosign sign keyless using the ambient GitHub OIDC token.
7. cosign attach sbom as an OCI referrer.
8. POST a WorkerImageSubmission to pipeline-cli's submission endpoint
   (feature:submission-endpoint) with the assembled fields:
   registry_ref, capability_tags, compatible_roles, sbom_ref, signed_by
   identity, provenance (repo URI, commit hash, run URL).

The worker manifest is a small declarative TOML file in the worker repo,
proposed shape:

```toml
[worker]
name = "implementer"
sdk_version = "0.3.0"
wire_protocol = "1.0"

[capabilities]
tags = ["code-writer", "frontier-reasoning"]
compatible_roles = ["engineering.implementer"]

[runtime]
kind = "subscribed"    # vs "invoked" if dagger path is added later
entrypoint = "implementer.main:run"
```

The manifest schema maps directly onto WorkerImageSubmission fields; the
workflow lifts manifest + build outputs into Submission shape.

Repo layout: monorepo with path-filtered triggers
(`workers/<name>/**` changes trigger that worker's release), scoped semver
tags (`implementer-v1.2.0`). When a worker eventually graduates to its own
repo, the workflow shape doesn't change.

Per-worker `.github/workflows/release.yml` becomes a one-screen file:

```yaml
name: release
on:
  push:
    tags: ['implementer-v*.*.*']
jobs:
  release:
    uses: hafeok/pipeline-cli/.github/workflows/release-worker.yml@v1
    with:
      worker_name: implementer
      manifest_path: workers/implementer/pipeline-worker.toml
    secrets:
      pipeline_submission_token: ${{ secrets.PIPELINE_SUBMISSION_TOKEN }}
```

A starter version of the reusable workflow ships alongside this Brief as
`release-worker.yml`.

---

## Feature feature:submission-endpoint

title: pipeline-cli endpoint that accepts WorkerImageSubmission POSTs

@motivated_by brief:worker-distribution-slice-1

An authenticated HTTP endpoint on pipeline-cli (`POST /submissions`) that
receives WorkerImageSubmission artifacts from worker repo CI. Validates
the submission against the WorkerImageSubmission SHACL shape, writes it
into the orchestration graph as an initial-request artifact through
GraphWriter, and emits a dispatch event for the WorkerCurator role.

Authentication: each worker repo has a `PIPELINE_SUBMISSION_TOKEN` secret
issued by pipeline-cli, scoped to that repo's identity. Slice 1: tokens
are long-lived and rotated manually. Slice 3+: tokens rotate via the same
mechanism that handles worker bearer tokens.

The endpoint is the orchestration system's only producer-side intake for
WorkerImages. All other worker artifacts (the WorkerImage itself, the
ConformanceAudit, the binding policy) are produced inside the orchestration
system by its roles.

---

## Feature feature:litellm-proxy-deployment

title: LiteLLM proxy as the LLM-call substrate for all workers

@motivated_by brief:worker-distribution-slice-1
@addresses_decision adr:litellm-as-llm-proxy-slice-1

A LiteLLM proxy deployment that workers route every LLM call through.
LiteLLM holds the actual provider API keys (Anthropic, OpenAI, Scaleway,
etc.); workers hold only a LiteLLM virtual key scoped to specific model
groups.

Slice 1 deployment shape:

- LiteLLM runs locally on the operator's machine (or in a sidecar container)
  on a known port — default `localhost:4000`.
- A `config.yaml` declares model groups and their backing providers, e.g.:

  ```yaml
  model_list:
    - model_name: frontier-reasoning
      litellm_params:
        model: anthropic/claude-opus-4-5
        api_key: os.environ/ANTHROPIC_API_KEY
    - model_name: fast-cheap
      litellm_params:
        model: anthropic/claude-haiku-4-5
        api_key: os.environ/ANTHROPIC_API_KEY

  general_settings:
    master_key: os.environ/LITELLM_MASTER_KEY
    database_url: os.environ/LITELLM_DB_URL  # optional; for spend tracking persistence

  litellm_settings:
    callbacks: ["pipeline-cli-telemetry"]   # POSTs to pipeline-cli's /llm-call-telemetry
  ```

- The model_name values are the framework's capability tags. Workers calling
  LiteLLM with `model="frontier-reasoning"` get routed to whatever provider
  + model that group is bound to. New providers / models / fallbacks land as
  config edits, not code changes.
- A virtual key is issued (via LiteLLM's `/key/generate`) at startup,
  scoped to the configured model groups, with a budget appropriate to slice
  1 (low; this is local dev). The key is written into the operator's local
  config so `pipeline-cli workers run` can inject it into worker containers.
- LiteLLM's logging callback (`pipeline-cli-telemetry`, implemented as a
  custom callback class) POSTs every call's telemetry — tokens, latency,
  cost, model, provider, fallback chain, retry count — to pipeline-cli's
  `/llm-call-telemetry` reconciliation endpoint, indexed by the
  `ddd_session_id` metadata that workers propagate per the SDK Brief's
  feature:provider-abstraction.

Operational scope:

- Slice 1: one LiteLLM instance per operator. Single tenant. Master key and
  virtual keys held in local env config.
- Slice 2: LiteLLM with persistent spend tracking (DB-backed); virtual keys
  issued per WorkerImage rather than shared.
- Slice 3+: multi-tenant deployment, per-tenant model groups, hardened
  authn for the master key. Excluded from slice 1 per
  feature:multi-tenant-litellm exclusion.

What we deliberately don't build:

- Provider adapters in our own SDK (LiteLLM has them all).
- A key vault (LiteLLM's virtual key system covers slice 1; secrets manager
  for slice 2+ if needed).
- Cost tracking infrastructure beyond what LiteLLM provides (we reconcile
  into session records but LiteLLM is the authoritative cost source per
  adr:litellm-as-llm-proxy-slice-1).

---

## Feature feature:manual-runtime-stance

title: pipeline-cli workers run subcommand and env-based secret handling

@motivated_by brief:worker-distribution-slice-1
@addresses_decision adr:no-supervisor-slice-1
@addresses_decision adr:secrets-via-env-slice-1

A `pipeline-cli workers run <worker-image-id>` subcommand that:

1. Looks up the qualified WorkerImage by ID in the orchestration catalog,
   resolves its registry_ref.
2. Pulls the image via docker (or podman; same CLI surface).
3. Reads environment variables from a local config file
   (`~/.pipeline-cli/workers.env` or path overridable by flag) containing:
   - `PIPELINE_ENDPOINT` — the SSE endpoint URL
   - `PIPELINE_TOKEN` — the worker's bearer token for pipeline-cli auth
   - `LITELLM_BASE_URL` — the LiteLLM proxy URL (defaults to
     `http://localhost:4000`)
   - `LITELLM_API_KEY` — the worker's LiteLLM virtual key, scoped to
     specific model groups
4. Invokes `docker run` with those vars set, with `--rm` for clean exit and
   stdout/stderr attached to the calling terminal.

No daemon, no restart, no autoscale. The human IS the supervisor in slice 1;
they read the orchestration system's binding state and start one process per
capability tag they want covered.

Secret trust model: the operator's local env config holds the pipeline-cli
bearer token and the LiteLLM virtual key. Provider API keys (Anthropic,
OpenAI, Scaleway) live in LiteLLM's config, not in worker env. Workers
cannot leak provider keys they never had. Acceptable because slice 1 is
single-tenant and runs on the operator's machine. Trust model breaks the
moment multiple operators, untrusted worker images, or remote hosting
enter — addressed by feature:multi-tenant-litellm (slice 3+) and
feature:secrets-manager-integration (slice 2+, alternative).

Slice 2-3 progression: `pipeline-cli workers compose` generates a
docker-compose.yml from current eligibility + binding state. Slice 4+: real
WorkerSupervisor service. Both excluded from this Brief.

---

## Acknowledgement ack:slice-1-single-tenant

@motivated_by brief:worker-distribution-slice-1

Slice 1 assumes one orchestration deployment, one operator, one trust list.
Multi-tenant concerns (per-tenant capability tag namespaces, per-tenant trust
lists, per-tenant Curators) are deferred. The discipline doesn't break under
multi-tenancy — it just gains a tenancy axis on every relevant artifact —
but slice 1 doesn't need it.

---

## Acknowledgement ack:manual-curator-decisions

@motivated_by brief:worker-distribution-slice-1

The WorkerCurator role is human-filled at Level 0 autonomy in slice 1.
Measurement evidence to graduate it to higher levels doesn't exist yet
because no WorkerImages have been admitted yet. This is structurally the same
place every role starts; the framework handles it.

---

## Acknowledgement ack:conformance-corpus-not-yet-exists

@motivated_by brief:worker-distribution-slice-1
@references feature:automated-conformance-replay   (excluded; slice 2+)

A conformance corpus is a set of historical bundles with known-good artifacts
that a candidate WorkerImage can be replayed against to verify its claims.
Slice 1 has neither historical bundles (the system hasn't run long enough)
nor known-good artifacts (no human-curated reference set). Slice 1
substitutes a manual Curator review; building the corpus is ongoing work as
production sessions accumulate, and automated replay audit comes when enough
corpus exists to be meaningful.

---

## Acknowledgement ack:env-var-secret-trust-model

@motivated_by brief:worker-distribution-slice-1
@references adr:secrets-via-env-slice-1
@references adr:litellm-as-llm-proxy-slice-1

Slice 1 holds two secrets on the operator's machine: the pipeline-cli bearer
token (worker → harness auth) and the LiteLLM virtual key (worker → LiteLLM
auth, scoped to specific model groups). Provider API keys (Anthropic, OpenAI,
etc.) live in LiteLLM's config, separate from worker env. Workers cannot
leak provider keys they never had.

The narrower trust surface — virtual keys with budgets and scope, not raw
provider keys — is one of the wins from pulling LiteLLM into slice 1 rather
than deferring it. The operator's local env config is still a single-point
secrets store and the trust model still breaks under multi-operator,
untrusted-image, or remote-host conditions. Those concerns are addressed
by feature:multi-tenant-litellm (slice 3+) and
feature:secrets-manager-integration (slice 2+ alternative).

Concrete env-var trust model for slice 1:

- `PIPELINE_TOKEN` — operator possesses; rotated by pipeline-cli.
- `LITELLM_API_KEY` — operator possesses; can be revoked by re-issuing.
- `ANTHROPIC_API_KEY` (and other provider keys) — only in LiteLLM's config,
  never in worker env. Rotated by editing LiteLLM's config and restarting
  the proxy.

---

## ADR adr:worker-image-mirrors-model-catalog

@decides_for feature:worker-image-artifact-type

The Model catalog in impl doc §9 already solves the analogous problem for LLM
models: a catalog of identity-versioned entities with capability tags,
eligibility status, and provenance from registration evidence; policy binds
capability tags to specific catalog entries; new entries enter via a
registration audit.

Decision: mirror the Model catalog shape for WorkerImage. Same field
vocabulary (identity, version, capability tags, eligibility status,
provenance), same registration discipline (audit → admit → bind), same policy
mechanism (capability-tag-to-entry binding).

Avoids reinventing concepts the framework already has language for. Future
cross-cutting work (an "Eligible" abstract supertype shared by Model and
WorkerImage; aggregate queries across both) becomes possible because the
shapes are aligned.

---

## ADR adr:oci-format-over-wheels-and-custom

@decides_for feature:oci-packaging-conventions

Three formats considered for worker packaging:

- **Python wheels**: no system deps captured, no signed-content model fitting
  the catalog discipline, language-specific. Reject.
- **OCI containers**: capture all deps, universal signing layer (sigstore),
  language-agnostic, registry infrastructure exists everywhere. Accept.
- **Custom DDD bundle format**: invented complexity, no ecosystem. Reject.

OCI also leaves room for the Dagger option later (Dagger modules ride on OCI),
which keeps that door open without committing.

---

## ADR adr:cosign-keyless-via-github-oidc-slice-1

@decides_for feature:cosign-signing-flow

Originally this slice deferred Fulcio + Rekor keyless signing to slice 2 under
the assumption that "no CI is in the picture yet." With GitHub Actions as the
chosen release driver, CI IS the picture from day one and the simplification
of local key-based signing doesn't pay off — GitHub OIDC token → Fulcio cert
→ cosign sign is exactly as little code as a local key flow, with much
stronger properties.

Decision: keyless signing via cosign + GitHub OIDC from slice 1. The Rekor
transparency log entry is referenced from the Submission. The orchestration
system maintains a trust list of permitted Fulcio identities (matched by
GitHub repo, workflow path, and tag pattern).

Local key-based signing remains a supported fallback for development outside
GitHub Actions but isn't the primary path.

Trade: dependency on Fulcio + Rekor availability for releases. Both are
operated by the OpenSSF / Sigstore project with strong SLA; offline signing
mode exists for emergencies. Acceptable.

---

## ADR adr:sbom-as-oci-referrer

@decides_for feature:sbom-attachment

SBOM placement options:

- **Embedded in the image** (file under /usr/share/sbom): works, but you must
  pull the whole image to read the SBOM. Wasteful at catalog-scan time.
- **Stored as a sibling artifact** (separate registry entry): needs additional
  convention for finding it; couples weakly to the image.
- **Attached as an OCI referrer per OCI v1.1**: standard mechanism, the
  registry returns "what's attached to this digest" without pulling the image
  content. Tooling exists (cosign attach, syft).

Decision: OCI referrer. Standard, query-efficient, well-tooled.

---

## ADR adr:capability-tags-as-oci-labels

@decides_for feature:oci-packaging-conventions

Capability tags must be discoverable from the OCI manifest without pulling the
image (for catalog operations and policy evaluation). OCI labels are the
natural carrier: `ddd.capability-tag.frontier-reasoning=true` per claimed tag.

This is a soft claim — the Curator still verifies the labels against the
actual worker behavior during conformance audit. But for the catalog's
shallow operations (find images claiming tag X), labels suffice.

---

## ADR adr:reusable-workflow-vs-per-repo

@decides_for feature:release-workflow

GitHub Actions supports reusable workflows. Options:

- **Reusable workflow centrally hosted**, called by each worker's tiny per-repo
  workflow. Worker repos carry one short file; the canonical release flow is
  versioned in one place. Updates to the flow happen once.
- **Per-repo workflows duplicated**: each worker repo owns its release flow
  independently. Easy to customize per worker, hard to keep consistent.
- **Composite actions instead of reusable workflows**: composable building
  blocks rather than a full workflow. More flexible but more boilerplate per
  consumer.

Decision: reusable workflow centrally hosted in pipeline-cli's repo (or a
dedicated workflows repo), called from per-worker `release.yml` files with a
manifest path and worker name. Versioned via tag (`@v1`, `@v2`). Worker repos
pin to a version, opt into updates explicitly.

The reusable workflow is itself a versioned artifact with provenance and
change history; revisions to it follow the same discipline as anything else.

---

## ADR adr:manual-conformance-slice-1

@decides_for feature:worker-curator-role

Conformance audit (replay against a corpus of historical bundles) is the gold
standard for verifying a worker actually does what its labels claim. Slice 1
cannot run automated audit because no corpus exists yet.

Decision: slice 1 uses manual Curator review. The Curator reads the
Submission, inspects the source repo, optionally pulls and runs the image
against ad-hoc test inputs, and produces a verdict based on judgment. The
WorkerImage's `conformance_audits` field is populated with a single
ConformanceAudit artifact of class `manual-review` carrying the Curator's
notes.

Slice 2+ replaces manual review with automated replay against the
accumulating corpus. The artifact shape doesn't change; the ConformanceAudit's
class field distinguishes `manual-review` from `automated-replay`.

---

## ADR adr:no-supervisor-slice-1

@decides_for feature:manual-runtime-stance

A WorkerSupervisor service that the orchestration system instructs to spawn
and scale worker instances is the right long-term answer. Slice 1 doesn't
need it: a human runs `pipeline-cli workers run <image-id>` for each worker
they want active.

Decision: defer the Supervisor to slice 4+. Slice 1 ships only the `workers
run` subcommand and a manual runtime stance.

Risk this defers: orchestration cannot autonomously bring workers up or down
based on dispatch demand, so capability tags with no running worker process
result in dispatches that escalate to humans. Acceptable in slice 1 because
the operator IS the human running workers and the escalation loop is fast.

Progression: slice 2-3 adds `pipeline-cli workers compose` that generates a
docker-compose.yml from current binding state (restart policy via compose,
still no autonomous decisions). Slice 4+ ships the real Supervisor.

---

## ADR adr:secrets-via-env-slice-1

@decides_for feature:manual-runtime-stance

Worker processes need two secrets: the pipeline-cli bearer token and the
LiteLLM virtual key. Provider API keys (Anthropic, etc.) are NOT in this
list — they live in LiteLLM's config per adr:litellm-as-llm-proxy-slice-1,
not in worker env. Options for the two secrets workers do need:

- **Env vars at container start, sourced from local config file**: simplest,
  works with `docker run`, no infrastructure dependency. Visible in process
  env and `docker inspect`; OK for single-operator local deployments.
- **Docker/Kubernetes secrets**: better than env vars but tied to specific
  runtime; doesn't transparently work across docker / podman / k8s.
- **External secrets manager** (Vault, AWS/GCP Secret Manager): production
  answer for multi-tenant; introduces a runtime dependency.

Decision: env vars from local config for slice 1. The `pipeline-cli workers
run` subcommand reads `~/.pipeline-cli/workers.env` and passes vars to docker
via `--env-file`. Acceptable under ack:env-var-secret-trust-model — the
trust surface is narrower than it would be without LiteLLM (no raw provider
keys in worker env).

Secrets manager option tracked under feature:secrets-manager-integration
(slice 2+ alternative when scope grows).

---

## ADR adr:litellm-as-llm-proxy-slice-1

@decides_for feature:litellm-proxy-deployment
@references brief:pipeline-worker-slice-1   (SDK Brief)
@references adr:litellm-as-provider-substrate   (in SDK Brief)

Originally this Brief deferred the LLM proxy to slice 2 under the assumption
that building one was significant engineering work. Evaluation surfaced
LiteLLM as a mature open-source proxy that solves the same problem: unified
OpenAI-shaped API across providers, virtual key management, spend tracking,
logging callbacks, fallbacks, retries. Using LiteLLM, the engineering cost
of "running the proxy" drops to "operating a configured service." The
deferral no longer pays off.

Decision: LiteLLM is the LLM proxy from slice 1. Workers route every LLM
call through it. Provider API keys live in LiteLLM's config; workers hold
only a LiteLLM virtual key. The SDK's Provider layer is a thin LiteLLM
client (per the SDK Brief's adr:litellm-as-provider-substrate).

Authoritative source-of-truth split:
- Our session record (pipeline-cli's orchestration graph) is authoritative
  for: provenance, bundle hash, role, motivational origin, downstream
  consequences. Workers report their own telemetry in completion events.
- LiteLLM is authoritative for: rate limit state, fallback decisions made
  during a call, the cost figure (LiteLLM sees actual provider pricing).
  LiteLLM's logging callback POSTs telemetry to pipeline-cli's
  reconciliation endpoint; the session record absorbs the cost figure and
  flags drift against the worker's self-reported telemetry as a fitness
  signal.

OpenAI-shaped API at the worker layer is acceptable: it's the de facto
standard, and provider-specific features (Anthropic tool use, etc.) pass
through via LiteLLM's `extra_body` parameter.

Why this isn't framework-lock-in (the way LangChain/AutoGen would have
been): LiteLLM is a wire-level translator/proxy, not a composition
framework. It doesn't impose how work is structured or how agents compose.
Analogous to accepting `requests` as an HTTP layer; not analogous to
accepting Rails as an app framework.

Alternatives considered:

- **Build our own LLM proxy from scratch** (the original plan). Duplicates
  commoditized work; no observability, routing, or cost-tracking wins.
  Rejected.
- **OpenRouter or similar SaaS proxy.** Same architectural fit but
  introduces a third-party runtime dependency on the critical path.
  Self-hosting LiteLLM keeps the dependency at the open-source library
  level. Rejected for the SaaS path; LiteLLM-as-self-hosted accepted.
- **LiteLLM as Python SDK only (no proxy server).** In-process use per
  worker instead of a separate proxy. Loses centralized key management,
  centralized logging, and the ability for multiple worker processes to
  share one configured deployment. Rejected; proxy is the right shape.
- **External secrets manager for raw provider keys per worker.** Workers
  still call providers directly; secrets manager handles key rotation.
  Simpler to adopt; doesn't get call-layer scoping or cost-source
  unification. Tracked as feature:secrets-manager-integration (excluded;
  alternative path if LiteLLM proves problematic for some specific
  reason).

Slice 1 ships LiteLLM with one model group (Anthropic via the provider's
API). Adding OpenAI / Scaleway / Bedrock / Vertex / etc. is a LiteLLM
config edit, not a code change anywhere in pipeline-cli or the worker SDK.

---

## ADR adr:dagger-deferred

@decides_for brief:worker-distribution-slice-1

Dagger (dagger.io) was evaluated as an alternative runtime model for workers:
workers as Dagger functions, content-addressed, OCI-distributed by default,
with the Dagger engine handling invocation rather than the SSE/POST contract
from the SDK Brief.

The fit is asymmetric. Dagger's function model is short-lived RPC; the current
worker design is event-subscribed and long-running. Dagger gains:
multi-language SDKs, free content-addressed caching, baked-in OCI
distribution. Costs: substrate dependency (open-source but commercially
driven), cold-start per dispatch, inverted invocation flow, stateless worker
shape only, adopted Dagger conventions.

Decision: stay with OCI + sigstore + the SSE/POST SDK contract. Dagger is not
adopted in slice 1.

Doors left open:
- **Hybrid runtime model.** Dagger as a second runtime stance alongside
  subscribed workers, distinguished by a `runtime_kind` field on WorkerImage
  (`subscribed` vs `invoked`). Reconsider when a worker shape appears that
  genuinely prefers RPC over subscription (stateless pure-execution,
  high-fanout classifiers).
- **Dagger as conformance-audit runtime.** Even if production dispatch stays
  on SSE/POST, Dagger's content-addressed caching makes it a clean fit for
  conformance-replay infrastructure. Worth evaluating when the conformance
  corpus exists and automated replay is being built (slice 2+).

Both are listed as feature exclusions in this Brief, deliberately, so
re-entering the decision later finds the prior reasoning attached.

---

## Open questions

1. **Catalog physical location.** Does the WorkerImage catalog live in
   pipeline-cli's main Oxigraph (same store as Model catalog and session
   records), or its own dedicated artifact store? Lean: same store, different
   named-graph namespace. Confirm during implementation.

2. **WorkerCurator bundle assembly.** What's the curated bundle query? The
   role needs to see the Submission, the identity verdict, the SBOM reference,
   current policy, and overlapping existing WorkerImages. The query shape
   isn't novel but needs to be authored.

3. **Conformance corpus genesis.** How does the corpus start accumulating?
   The natural path: every production session writes a (bundle, artifact)
   pair tagged by role; some subset gets manually marked as "known good" by
   the Curator over time. Slice 2 makes this explicit; slice 1 just lets the
   data accumulate.

4. **LiteLLM deployment topology beyond slice 1.** Slice 1 runs LiteLLM
   locally per operator. Slice 2+: shared LiteLLM (one per orchestration
   deployment), persistent spend tracking DB, per-WorkerImage virtual keys
   instead of one shared key. The progression is straightforward; sequencing
   within slice 2 is open.

5. **Cost-reconciliation drift threshold.** LiteLLM reports cost per call;
   workers report local telemetry; pipeline-cli reconciles. What drift
   threshold flags a fitness function violation — exact match required,
   1% tolerance, 5% tolerance? Depends on the precision LiteLLM's pricing
   tables actually maintain. Validate empirically once slice 1 runs.

6. **LiteLLM master key custody.** Slice 1 holds it in the operator's local
   env config (`LITELLM_MASTER_KEY`). Slice 2+ hardening: HSM-backed,
   rotated automatically, never on disk. Out of scope for slice 1 but worth
   tracking.

5. **Dagger-as-conformance-runtime revisit.** Track in adr:dagger-deferred
   with a forward reference to the slice that re-evaluates. Tentatively slice
   2 when the corpus exists.

6. **Identity rotation and revocation.** What happens when a signing identity
   is compromised or rotated? The trust list mechanism handles it (remove the
   identity), but qualified WorkerImages signed by the removed identity need
   a policy: auto-deprecate, require resignature, or grandfather. Deferred.

7. **GitHub Actions vendor lock.** The release flow is currently GitHub-shaped.
   GitLab CI, Buildkite, etc. would work with the same primitives (OIDC, cosign,
   ghcr-equivalent) but the reusable-workflow file is platform-specific.
   Cross-CI portability is a slice 3+ concern if/when other CI hosts become
   relevant.
