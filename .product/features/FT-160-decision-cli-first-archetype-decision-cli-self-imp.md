---
id: FT-160
title: 'decision-cli: First archetype — decision-cli self-implementation — with backfill of FT-139..FT-144 and witnessed seam audits'
phase: 5
status: planned
depends-on:
- FT-147
- FT-148
- FT-149
- FT-150
- FT-152
- FT-158
adrs:
- ADR-082
- ADR-084
tests: []
domains:
- api
- data-model
- observability
- security
domains-acknowledged: {}
---

## Description

The **first archetype** — `dec:archetype:decision-cli-self-implementation`. Authors the ApplicationContract (Rust + vertical slice + SDP at `oxi-events` / `core` / `features`), the InfrastructureContractTemplate (LiteLLM proxy + OCI registry + Scaleway/Anthropic capability bindings), the first InfrastructureContractInstance (the live decision-cli repo's actual deployment), and the SeamAudits required by [ADR-084](ADR-084). Backfills the existing TaskTypes ([FT-139](FT-139)..[FT-144](FT-144)) into the archetype via the FT-150 typed-artifact migration.

This is the load-bearing prototype of the archetype layer. Per [ADR-082](ADR-082) §7 + §Status, the layer's correctness is unproven until a first archetype regenerates a known-good instance with the seam audits catching at least one drift the type system would otherwise have missed. The witnessed drift between code-writer's `LITELLM_BASE_URL` assumption and the LiteLLM-proxy slice ([FT-096](FT-096) / [ADR-053](ADR-053) / [ADR-064](ADR-064)) is the candidate target: if `app-config-matches-iac-outputs` catches it, the layer is proven; if not, the framework needs work before a second archetype lands.

## Functional Specification

### Inputs

- The full FT-147..FT-159 substrate.
- The live decision-cli repo at HEAD (the regeneration target for the regression test).
- The TaskType feature_specs FT-139..FT-144 (migration source).
- The LiteLLM proxy + worker-distribution Bicep templates from FT-096 (the infrastructure template substrate).

### Outputs

**Archetype directory** at `forge/archetypes/decision-cli-self-implementation/`:

```
forge/archetypes/decision-cli-self-implementation/
├── archetype.yaml                         # status: candidate (per ADR-085)
├── application/
│   ├── contract.md                        # the six required conventions
│   └── conventions/
│       ├── language-runtime.md            # "Rust 2021, workspace crates, edition 2021"
│       ├── layering-rule.md               # SDP: oxi-events <- core <- features; binary main.rs is wiring only
│       ├── feature-organisation.md        # vertical slices under crates/decision-cli/src/features/ft_NNN_<title>/
│       ├── persistence.md                 # oxigraph + named graphs + SHACL chokepoint at GraphWriter
│       ├── endpoint-convention.md         # CLI subcommand + MCP twin per ADR-029 + ADR-081 list/show totality
│       └── cross-cutting.md               # auth (none — local CLI), error handling (thiserror + anyhow per scope), logging (tracing crate)
├── infrastructure/
│   ├── contract.template.md               # slots: LLM proxy, OCI registry, capability providers, observability
│   ├── conventions/
│   │   ├── naming.md                      # resource naming conventions for the workspace
│   │   ├── networking.md                  # local + cloud network stance
│   │   └── identity.md                    # capability-binding identity model
│   └── instances/
│       └── live-repo/
│           └── infrastructure.contract.md # the actual values for the live repo (LITELLM_BASE_URL, OCI registry path, ...)
├── task-types/
│   ├── application/
│   │   ├── add-judge-worker/              # symlinks or refs to the FT-139 artifact
│   │   ├── add-author-worker/             # FT-140
│   │   ├── add-artifact-type/             # FT-141
│   │   ├── add-cli-subcommand/            # FT-142
│   │   ├── extend-planner-classifier/     # FT-143
│   │   └── extend-role-catalog-seed/      # FT-144
│   └── infrastructure/                    # initially empty for v1; reserved for future Bicep-emitting TaskTypes
├── audits/
│   ├── archetype/
│   │   ├── slice-conforms-to-clean-architecture.md     # SDP at oxi-events / core / features
│   │   ├── endpoint-contract-test-alignment.md          # list/show totality, MCP twin parity
│   │   └── bicep-conforms-to-naming.md                  # placeholder until infra TaskTypes land
│   └── seam/
│       ├── app-config-matches-iac-outputs.md            # CATCHES the LITELLM_BASE_URL drift
│       ├── app-identity-matches-iac-roles.md            # capability tag → granted endpoint match
│       └── app-resource-expectations-met.md             # every capability the role catalog needs is provisioned
├── EVIDENCE.md                              # coverage, variance, contract invariance, regression results
└── EXTRACTION-REPORT.md                     # the FT-156 extractor's report (or hand-authored equivalent for v1)
```

**ApplicationContract bodies (the six conventions):**

Each convention file states the rule + the audit that checks it. Examples:

- `layering-rule.md`: "decision-cli enforces SDP at three boundaries: (1) `crates/oxi-events/` depends only on its declared deps and never on `decision-cli`; (2) `core/` depends on nothing in `features/`; (3) each `features/ft_NNN_*/` depends on `core/` but never on another feature. The check: `cargo deps` + a regex against `use` statements. Script: `scripts/checks/sdp-boundaries.sh`."
- `feature-organisation.md`: "Each feature lives under `crates/decision-cli/src/features/ft_NNN_<title>/` with `mod.rs` and feature-internal modules. Check: every `FT-NNN` in product-cli with `status: complete | in-progress` has a matching directory. Script: `scripts/checks/feature-slice-presence.sh`."
- `endpoint-convention.md`: "Every list verb has a paired show verb registered in `cli_pairing.rs` (per ADR-081). Every CLI verb that mutates a graph artifact has an MCP twin (per ADR-029) UNLESS it is in the gating set (per ADR-085's promote/demote exclusion). Script: `scripts/checks/cli-mcp-parity.sh`."

**InfrastructureContractTemplate slots:**

- `llm-proxy` (required, satisfaction: capability-resolver invariant): legal_choices `[litellm-proxy]`; iac_outputs `[base-url, api-key-env-var]`.
- `oci-registry` (required, satisfaction: worker-image distribution): legal_choices `[github-ghcr, azure-acr]`; iac_outputs `[registry-url, push-credentials-env-var]`.
- `capability-providers` (required, satisfaction: capability-vocabulary invariant): legal_choices `[scaleway+anthropic, scaleway-only, anthropic-only]`; iac_outputs `[scaleway-api-key-env-var, anthropic-api-key-env-var]`.
- `observability` (optional, satisfaction: tracing-crate invariant): legal_choices `[stdout-tracing, jaeger, datadog]`; iac_outputs `[trace-endpoint-url]`.

**InfrastructureContractInstance `live-repo`:**

- `customer_id: "decision-cli-local-dev"`.
- `status: Frozen`.
- `slot_choices`:
  - `llm-proxy: "litellm-proxy"` — satisfaction_evidence: "LiteLLM proxy provisioned via FT-096; satisfies the capability-resolver invariant".
  - `oci-registry: "github-ghcr"` — satisfaction_evidence: "ghcr.io path declared in FT-088".
  - `capability-providers: "scaleway+anthropic"` — satisfaction_evidence: "Both providers wired per ADR-037".
- `iac_outputs`:
  - `name: "base-url"`, `value_shape: "url"`, `source_module: "litellm-proxy-bicep"`.
  - `name: "api-key-env-var"`, `value_shape: "secret-ref"`, `source_module: "litellm-proxy-bicep"`.
  - ... etc for each declared output.

**SeamAudit `app-config-matches-iac-outputs` — the load-bearing one:**

- `family: AppConfigMatchesIacOutputs`.
- `name: "app-config-matches-iac-outputs"`.
- `runner: "bash"`, `runner_args: "forge/archetypes/decision-cli-self-implementation/audits/seam/app-config-matches-iac-outputs.sh"`, `runner_timeout: 60s`.
- `monolith_bar: CandidateAuditWeak` initially; flips to `Passes` after regression evidence accumulates.
- The script implementation:
  1. Greps `crates/decision-cli/src/` + `workers/*/src/` for env-var reads (e.g., `env::var("LITELLM_BASE_URL")`, `os.environ["LITELLM_BASE_URL"]`).
  2. Reads the InfrastructureContractInstance's `iac_outputs` field set.
  3. Asserts: every env-var the application reads has a corresponding `iac_outputs.value_shape: secret-ref | url` entry in the instance.
  4. Exit 0 on pass; exit 1 with the missing env-var name on fail.

**The regression test (per ADR-084 §5):**

- Generate a regression-evidence record by temporarily renaming `LITELLM_BASE_URL` to `LITELLM_PROXY_URL` in the live `iac_outputs` (in a regression fixture, not the real contract).
- Run the seam-audit script against this fixture.
- Assert it exits 1 with `LITELLM_BASE_URL` named in stderr (the app reads `LITELLM_BASE_URL` but the iac_outputs no longer emits it).
- Record this as a `RegressionEvidence` artifact: `audit: seam:app-config-matches-iac-outputs`, `instance: instance:live-repo`, `drift_caught: "LITELLM_BASE_URL rename simulated; audit caught the unprovisioned read"`.
- Flip the SeamAudit's `monolith_bar: Passes` and link the RegressionEvidence.

**TaskType backfill via FT-150 migration:**

- The bootstrap loader from FT-150 reads each of FT-139..FT-144 and writes typed TaskType + Cell artifacts.
- This slice extends the loader to set `archetype: dec:archetype:decision-cli-self-implementation` on each.
- It also sets `conforms_to` per TaskType: e.g., `add-judge-worker` conforms_to `[language-runtime, layering-rule, feature-organisation, persistence, endpoint-convention]` (the witnessed FT-139 cluster reads each of these conventions implicitly; this slice makes them explicit).

**EVIDENCE.md:**

- `archetype_layer_estimate: 0.6` (rough estimate — 6 TaskTypes cover the witnessed pattern; ~40% of features have been domain-layer or unmatched).
- `application_contract_held_invariant: true` (decision-cli is a single instance; trivially invariant; flag this in coverage_note).
- `instance_variance: low` (one instance).
- `seam_regression_results: [<regression-evidence-iri>]`.
- `coverage_note: "Single-instance archetype. Three-instance threshold for promotion not met; archetype stays at candidate. The framework's contract format is validated by this archetype's ability to express its bindings; the next archetype is the test of cross-customer reuse."`

**Test coverage:**

- Round-trip: archetype + contract + template + instance all written and read back; equal.
- E102 absent: archetype has 3 seam audits → no E102.
- W104 absent: archetype has `instance_variance: low` and 1 instance → ADR-085's first evidence requirement fails (≥3 instances); W104 does not fire.
- Seam audit catches witnessed drift: the regression test from above; audit exits 1 with the expected diagnostic.
- TaskType backfill: post-migration, each of FT-139..FT-144 has `archetype: dec:archetype:decision-cli-self-implementation` and non-empty `conforms_to`.
- ArchetypeAudit `slice-conforms-to-clean-architecture`: runs against the live repo; SDP boundaries hold; exit 0.
- `dec drive archetype decision-cli-self-implementation FT-X` (for some sample feature X that exists post-FT-160): plan surfaces; classifier matches; audits pass; assembly places artifact; report emitted.

### State

- **New on-disk:** the entire `forge/archetypes/decision-cli-self-implementation/` directory tree.
- **Modified on-disk:** the FT-150 bootstrap loader extended with the convention/archetype binding logic; `EVIDENCE.md` for the archetype.
- **Graph-resident:** Archetype, ApplicationContract, InfrastructureContractTemplate, InfrastructureContractInstance, SeamAudits × 3, ArchetypeAudits × 3, RegressionEvidence × 1 (after the regression test runs), TaskTypes × 6 (with archetype + conforms_to set) — all written via typed GraphWriter paths.

### Behaviour

1. **Manual authoring of the contract bodies + audit scripts** for v1. The pattern-extractor (FT-156) is not yet field-tested; this slice hand-writes the substrate as the first witness.
2. **FT-150 migration extended** to bind the existing TaskTypes to the new archetype.
3. **Regression test runs as part of the slice's TC**. The test simulates the LITELLM_BASE_URL rename and asserts the seam audit catches it.
4. **archetype.yaml + EVIDENCE.md serve as the source-of-truth** for the catalog entry; graph artifacts are written from them.
5. **Status stays `candidate`** until 3 instances + regression evidence on every seam audit + ADR-085 promotion gate.

### Invariants

- **Status starts as `candidate`.** Never `standard` from this slice.
- **All three required seam-audit families are present.** Non-negotiable per ADR-084 §3.
- **The witnessed drift case is the regression test.** If the seam audit cannot catch the LITELLM_BASE_URL rename, the slice does not ship — the framework needs work.
- **Application contract holds invariant.** Trivially true for one instance; the regression test against an amended instance proves the assertion is non-vacuous.

### Error handling

- **Seam audit script unrunnable on the live repo** → slice fails its TC; not shipped.
- **Regression test does not catch the simulated drift** → slice fails its TC; surfaces in the report that the framework's seam-audit format is incomplete; not shipped.
- **TaskType backfill conflicts (TaskType already has an archetype set)** → bootstrap loader logs warning + skips; idempotency preserved.
- **`forge/archetypes/decision-cli-self-implementation/` directory exists with conflicting content** → slice fails at directory creation; operator clean-up required.

### Boundaries

- **In scope.** The complete first-archetype substrate: directory + contracts + conventions + templates + instance + audits + EVIDENCE.md + EXTRACTION-REPORT.md (manual for v1); the regression test catching the LITELLM_BASE_URL drift; TaskType backfill via FT-150 extension; seven TCs.
- **Out of scope.** A second archetype (`Self-Service Portal (.NET / Azure)` from the briefs) — separate slice. Production deployment of the LiteLLM proxy via Bicep + an actual infrastructure TaskType — out of v1; the InfrastructureContractInstance.iac_outputs are declared but no infrastructure TaskType emits them yet (placeholder). Promotion to `standard` — never automatic; requires 3+ instances. Mining real customer repos — future use case; v1 archetype is decision-cli itself.

## Out of scope

- A second archetype (Self-Service Portal).
- Production Bicep-emitting infrastructure TaskTypes.
- Promotion to `standard` (requires 3+ instances).
- Mining customer repos.
- Multi-instance EVIDENCE roll-up across customer deployments.
