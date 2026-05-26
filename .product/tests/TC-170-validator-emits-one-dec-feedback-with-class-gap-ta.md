---
id: TC-170
title: Validator emits one dec:Feedback with class=gap targeted at the upstream catalog artifact, not the worker
type: scenario
status: unimplemented
validates:
  features:
  - FT-102
  adrs: []
phase: 1
---

## Claim

When the validator rejects a proposal, it emits exactly one `dec:Feedback` artifact **per natural upstream target** — the `dec:CapabilityReference`, `dec:OntologyDescription`, or `dec:VerificationEnvironment` that was the right place to register the missing fact. The feedback has `dec:class = "gap"`, `dec:fromActivity` set to the bundle-assembly activity (not the worker activity), and a body containing the violation detail plus a remediation suggestion.

## Scenarios

### Setup

- Temp `.dec/` with the TC-168/TC-169 catalog and the validator-injection seam.

### Scenario A — single missing dec subcommand → one feedback against the CapabilityReference catalog

Inject a proposal with one violation: `dec verify result inspect` not in `cli_surface.dec_subcommands`. Assertions:

- Exactly one `dec:Feedback` artifact is written to `.dec/feedback/`.
- `dec:class = "gap"`.
- `dec:target` resolves to a `dec:CapabilityReference` IRI **or** to the catalog category (`<catalog/capabilities>`) if no single CR is the natural target. The test pins the contract: when no CR exists for the command, the target is the category IRI; when one exists but is missing the subcommand, the target is that CR.
- `dec:fromActivity` is the bundle-assembly activity, NOT the worker dispatch activity. The test asserts this by walking PROV-O: `fromActivity prov:wasInformedBy <bundle-assembly>` should hold, NOT `<worker-dispatch>`.
- Body contains: the violation detail (`"step 0: dec subcommand 'dec verify result inspect' not in cli_surface"`), plus a remediation hint (`"add 'dec verify result inspect' to a CapabilityReference (e.g. dec catalog capability new CR-NNN --command 'dec verify result inspect' ...), then re-run 'dec verify graph generate FT-XXX'"`).

### Scenario B — missing SPARQL namespace → feedback against the OntologyDescription

Inject a proposal violating only via an unknown SPARQL namespace. Assertions:

- One Feedback emitted with `dec:target` resolving to the active `dec:OntologyDescription` (e.g. OD-001).
- Body contains the missing namespace IRI and the suggestion `"add namespace to OD-NNN's ontology body via supersession (dec catalog ontology supersede OD-001 --by OD-002 --new-file ...), then re-run"`.

### Scenario C — multiple violations across categories → one feedback per category

Inject a proposal with three violations: one unknown dec subcommand, one unknown SPARQL namespace, one unknown file path. Assertions:

- Exactly three `dec:Feedback` artifacts emitted: one against the CapabilityReference catalog, one against the active OntologyDescription, one against the env's `dec:VerificationEnvironment`. The validator deliberately does NOT fan out to one feedback per violation — it fans out per natural upstream target so the operator's inbox has one actionable item per catalog edit needed.

### Scenario D — feedback links to the rejected proposal for context

Each emitted Feedback carries an additional predicate (e.g. `dec:rejectedProposalRef`) referencing the proposal that triggered the rejection — so an operator looking at the feedback can find the full proposal in the activity log via PROV-O. Assertions:

- The predicate is present on each emitted Feedback.
- Resolving it (via `dec session show <activity>`) reveals the proposal contents in the session record.

### Scenario E — feedback routing reaches the bundle assembler role's inbox

After emission, the existing feedback routing subscription ([FT-029](FT-029)) should route the feedback to the bundle-assembler role's inbox (not the worker's inbox). Assertions:

- `dec feedback list --role bundle-assembler` returns the emitted feedback(s).
- `dec feedback list --role verify-graph-author` does NOT return them — the worker is not the responsible party per ADR-066 Rule 3.

## Runner

`bash tests/scripts/tc-170-validator-gap-feedback.sh`. Same injection seam as TC-169. Depends on FT-029 being implemented (it is — slice 2 completed). The test must assert via the existing `dec feedback list` verb, not by reading `.dec/feedback/` directly.

## Non-goals

- The feedback's downstream lifecycle (FT-027 covers that).
- Auto-amend triggered by the feedback (no auto-amend in this slice — operator-driven per ADR-066 Rule 3).
- The exact wording of the remediation hint beyond the asserted substrings (allow evolution without breaking this TC).
