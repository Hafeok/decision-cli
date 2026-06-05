---
id: ADR-081
title: CLI enumerate/lookup verb pairs are total — every IRI returned by list must resolve via show
status: accepted
features:
- FT-012
- FT-146
- FT-158
- FT-161
supersedes: []
superseded-by: []
domains:
- api
- observability
scope: cross-cutting
content-hash: sha256:a15440343c8bac72e25b0d77ce41ffa426b96c133ab84ce1c2928e4ea5e81d9b
---

**Status:** Proposed

**Context:**

decision-cli exposes paired CLI verbs for enumerating and inspecting graph-resident artifacts: `dec session list` / `dec session show`, `dec events tail` / `dec events since`, `dec feedback list` / `dec feedback show`, `dec verify env list` / `dec verify env show`, `dec verify graph list` / `dec verify graph show`, `dec loop list` / `dec loop show`. The same pairing exists for the `dec product *` surface absorbed from product-cli ([ADR-077](ADR-077)) — `feature list` / `feature show`, `adr list` / `adr show`, `test list` / `test show`.

The pattern is universal in [ADR-011](ADR-011)'s namespaced subcommand vocabulary: every class with an enumeration verb gets a per-instance lookup verb, and IRIs flow from the former into the latter (`dec session list` prints IRIs; operators pipe them into `dec session show`). This contract is implicit but never written down — and it has already failed silently.

The failure that motivated this ADR: `dec session list` happily returns IRIs for sessions produced by the verify-graph-runner code path ([FT-098](FT-098)), but `dec session show <iri>` reports `"no Session with IRI <...>"` for the same IRIs. The list SPARQL wraps every field read in `OPTIONAL` and only requires `?s a dec:Session`; the show SPARQL hardcodes `prov:used ?bundle`, `prov:used ?model`, `dec:featureId`, `dec:inStream`, plus the linked resources' `dec:contentHash` and `dec:modelVersion` as mandatory. Sessions written by code paths that don't populate the full slice-1 implementer shape drop out of show but stay in list. The list↔show queries diverged as new session-producing paths landed (FT-098 verify-graph-runner, [FT-062](FT-062) escalation chain, [FT-139](FT-139) cluster_dispatch, the planned [FT-146](FT-146) cluster cell sessions) and nobody re-verified the per-feature TCs on [FT-012](FT-012) — the original slice-1 CLI feature — which is "complete" with three TCs, none of which exercise `list` or assert list↔show consistency.

This is the same shape as the failure modes [ADR-014](ADR-014) governs: a property that holds across every feature in the system, that no individual feature_spec is the right place to assert, and that drift will silently violate if no fitness function watches it.

**Decision:**

Adopt as a cross-cutting invariant, enforced via `product verify --platform`:

> **For every enumeration verb `<noun> list` in the decision-cli CLI, the per-instance lookup verb `<noun> show <iri>` must exit 0 for every IRI returned by list. Equivalently: list and show share the same domain — list is a total enumeration of show's resolvable set.**

The same constraint applies symmetrically to `<noun> log <iri>` when a `log` verb exists for the noun (today: `dec session log`). It does **not** apply to verbs that take non-IRI arguments (`dec status`, `dec health`, `dec preflight FT-XXX`).

**Enforcement:**

1. **Registry.** A small registry under `crates/decision-cli/src/core/cli_pairing.rs` declares each `(noun, list_verb, show_verb, log_verb_opt)` tuple as data — not metadata, not docs. Adding a new list verb without registering it is detected by Rule 2 below.
2. **Platform TC.** A `runner: bash` TC backed by `scripts/checks/cli-list-show-totality.sh` walks the registry. For each tuple it: (a) invokes `dec <noun> list --limit 50 --format json` (the JSON output mode lands as a prerequisite — see FT-XXX in §Test coverage); (b) for every IRI in the result, runs `dec <noun> show <iri>` and asserts exit 0; (c) for nouns with a log verb, runs `dec <noun> log <iri>` and asserts exit 0. Any non-zero exit from a show or log call surfaces the offending `(noun, iri, exit_code, stderr_excerpt)` and fails the TC.
3. **Registry coverage check.** A second TC (`runner: bash`, `scripts/checks/cli-pairing-registry-coverage.sh`) greps the clap command tree for `<noun> list` patterns and asserts each one is present in the registry. New list verbs are forced into the registry at PR time, not at runtime discovery.
4. **Empty-store contract.** When the orchestration store is freshly initialised and the noun has no instances, list returns zero rows and the TC passes trivially. This is intentional — the property is universal over the *currently extant* IRIs, not over a synthetic fixture set.

**The query-shape rule:**

list and show must read from the **same canonical projection** of the class. Each noun owns a single projection function in `core::graph::<noun>` (e.g. `core::graph::session::project`) that returns the field set both verbs render from. list's SPARQL feeds the projection by enumerating `?s a dec:<Class>` and joining the projection per row; show's SPARQL feeds the projection by binding `?s` to the input IRI. Identical projection ⇒ identical domain. Divergent projection ⇒ this ADR's invariant breaks.

This makes the "make list permissive and show strict" failure structurally impossible — the projection cannot have a field that is `OPTIONAL` for list but required for show, because the same code emits both queries' projection clause.

**Exit-code contract:**

Same two-tier contract as [ADR-013](ADR-013):

- **Exit 0** — every list-emitted IRI resolves via show (and log where applicable). Empty registries pass trivially.
- **Exit 1** — at least one IRI failed to resolve. Diagnostic lines on stdout enumerate the `(noun, iri, verb, exit_code)` quadruples and a stderr excerpt for the first failure per noun.

No warn-band. The invariant is binary: either list and show agree or they don't.

**TC files:**

Two TCs land alongside the ADR:

- **Platform TC (this ADR).** Linked via `validates.adrs: [ADR-081]`; runs the totality check across every registered pairing. Reaches `product verify --platform`.
- **Registry coverage TC (this ADR).** Linked via `validates.adrs: [ADR-081]`; greps the clap tree for unregistered list verbs. Catches drift between code and registry.

Plus the per-feature TCs added to [FT-012](FT-012) (`dec session list` smoke + list↔show round-trip for the session noun specifically) — those are feature-level TCs, not cross-cutting, and live on FT-012's `tests:` array. They overlap with the platform TC by construction; the redundancy is deliberate. FT-012's per-noun TC catches the failure inside the feature's own verify pipeline (so `product verify FT-012` is honest); the cross-cutting TC catches it across every noun a future slice adds without anyone remembering to author the per-feature TC.

**Rationale:**

- **Operators paste IRIs.** The CLI promises that the output of one verb is valid input to its paired verb. Violating that promise is silent — the operator gets `"no Session with IRI"` and assumes the IRI is stale or the store is corrupted, when the truth is that the two verbs use different SPARQL shapes. This wastes debugging time and erodes trust in the inspection surface.
- **The bug class is structural, not behavioural.** This isn't a single-session edge case; it's a missing invariant. A test that asserts "show works for the implementer-shape session" passes forever even as new session producers add shapes that show doesn't handle. The invariant must be expressed over the class, not over a fixture, which is what the registry + walk-the-store enforcement gives us.
- **The same shape repeats per noun.** Sessions, events, feedback, verify envs, verify graphs, loop reports, and the product-cli surface all have the pattern. Authoring N feature-level TCs for N nouns is brittle (someone forgets one when adding a new noun) and leaves cross-noun regressions undetected. A registry-driven cross-cutting TC closes both.
- **Same machinery as [ADR-013](ADR-013) and [ADR-014](ADR-014).** Cross-cutting ADRs already carry their enforcement scripts and run through `product verify --platform`. No new infrastructure required.
- **Forces canonical projection.** The query-shape rule is the load-bearing part of the decision. Without it, the enforcement TC catches the symptom but the root cause — two diverging SPARQL bodies — is free to reappear. With it, list and show share a single projection function and the bug class becomes a compile error rather than a runtime drift.

**Rejected alternatives:**

- **Add tests per noun and call it done.** Closes the immediate gap but does nothing for future nouns. Every new list/show pair shipped without a matching test re-opens the door. The cross-cutting ADR + registry forces the protection forward.
- **Make list strict instead of show permissive.** Surface-level fix — list drops sessions it can't render, show keeps working. But operators still can't inspect partial-shape sessions, and the underlying drift between projection shapes persists. Rejected: hides the problem instead of fixing it.
- **Refactor show to query the union of all known shapes.** Same drift problem on the other side: every new shape requires updating show. Rejected: shifts maintenance burden without removing the duplication.
- **Lift the rule into SHACL — require every `dec:Session` to carry the full slice-1 projection.** Would force every session producer to write the full field set, which is wrong: verify-graph-runner sessions legitimately have no `prov:used` bundle (they're not LLM dispatches). Rejected: conflates "is a Session" with "is a session of a particular dispatch shape." The right rule is about CLI surface consistency, not ontology rigidity.
- **Document the invariant in CLAUDE.md or CONTRIBUTING.md.** Rules in prose are not rules — they're aspirations. [ADR-014](ADR-014) is explicit that fitness functions live as cross-cutting ADRs with TCs and enforcement scripts. Rejected: this ADR is exactly the kind of thing ADR-014 governs.

**Test coverage:**

- **TC for this ADR (platform).** `runner: bash` walks the registry; asserts list↔show↔log totality across every registered pair. Linked via `validates.adrs: [ADR-081]`. Reaches `product verify --platform`.
- **TC for this ADR (registry coverage).** `runner: bash` greps the clap tree; asserts every `<noun> list` clap subcommand appears in `cli_pairing.rs`. Linked via `validates.adrs: [ADR-081]`. Reaches `product verify --platform`.
- **TC on [FT-012](FT-012) (session-noun smoke).** `dec session list` against a populated store returns ≥1 row with all four columns populated. Linked via `validates.features: [FT-012]`.
- **TC on [FT-012](FT-012) (session-noun round-trip).** For every IRI in the output of `dec session list --limit 50`, `dec session show <iri>` exits 0 and `dec session log <iri>` exits 0. Linked via `validates.features: [FT-012]`. The session-noun specialisation of the cross-cutting invariant.

**Implementation prerequisites:**

- JSON output mode on the list verbs (`--format json` flag). Slice-1 list output is human-formatted whitespace ("(unknown-time)" placeholders, IRI in tail position). The platform TC needs structured output to extract IRIs reliably. Tracked as a separate small feature; if not yet shipped, the TC can fall back to a tab-delimited extraction (`iri=` token) but the JSON path is the clean one.
- The canonical projection refactor in `core::graph::session` (and the equivalent module per noun, where missing). This is the load-bearing change that prevents recurrence; it does not require a separate ADR but does require a feature_spec to track the work.

**Derivation:** This ADR sits alongside [ADR-013](ADR-013) (code structure) and [ADR-014](ADR-014) (fitness functions as ADRs) as a third cross-cutting quality ADR. It governs CLI surface consistency, not source structure or code metrics; the enforcement machinery (`runner: bash` script under `scripts/checks/`, `product verify --platform`) is identical.
