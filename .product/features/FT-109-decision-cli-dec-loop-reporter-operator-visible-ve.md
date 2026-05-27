---
id: FT-109
title: 'decision-cli: dec loop reporter — operator-visible verify→re-fix audit trail'
phase: 3
status: complete
depends-on:
- FT-012
- FT-107
- FT-108
adrs:
- ADR-004
- ADR-024
- ADR-026
tests:
- TC-191
- TC-192
- TC-193
- TC-194
domains: []
domains-acknowledged: {}
---

## Description

After [FT-107](FT-107) and [FT-108](FT-108) closed the verify → re-fix loop for both worker roles, the data trail is complete but invisible. Every `dec:Feedback` carries the full PROV-O chain — `source_session` (the failing verify run), `source_artifact` (the TC that regressed), `receiving_session` (the worker dispatch), `addressing_artifact` (the new graph or CodeChange that resolved it), `lifecycle_state` (open vs closed) — but the operator has to walk these by hand with `dec feedback show <iri>` to reconstruct what happened.

This slice adds two operator-facing views that roll the chain up:

- **`dec loop show <FT-NNN>`** — chronological audit trail for one feature: every defect feedback for any of its TCs, with timestamps, state, evidence excerpt, and the artifacts that emitted / received / addressed each one.
- **`dec loop list`** — overview across all features: open vs closed defect-feedback counts per feature, sorted by open-count descending. The "what's still broken" dashboard.

Plus one mechanical fix the views surface naturally:

- **Worker-dispatch sessions materialise as `dec:Session` artifacts.** Today the verify-graph-author and code-writer dispatch paths emit `activity/...` IRIs that get attached as `receivingSession` on feedback but aren't `rdf:type dec:Session`. They don't appear in `dec session list` and `dec session log` can't walk them. The reporter would render them anyway via `receivingSession`, but materialising them properly lets the existing `dec session` surface cover both halves of the loop.

One subcommand → one slice — `dec loop show` and `dec loop list` are siblings of a new `loop` verb, sharing a chain-walker; the worker-session change is a mechanical bug-fix on the dispatch path that the reporter exposes. Together they're tight enough for one slice.

## Functional Specification

### Inputs

#### 1. `dec loop show <FT-NNN>`

```
dec loop show <FT-NNN> [--format text|json]
```

Walks the orchestration store for every `dec:Feedback` whose `dec:sourceArtifact` is one of FT-NNN's TC IRIs (`https://decision-cli.dev/ns/tc/TC-NNN`). For each feedback, projects:

| Field | Source |
|---|---|
| feedback_iri | the feedback subject |
| created_at | derived from `source_session`'s `prov:startedAtTime` (or — if absent — the dump's insertion order tiebreaker) |
| class | `dec:feedbackClass` |
| target_role | `dec:targetRole` |
| state | `dec:lifecycleState` |
| evidence | `dec:evidence` (truncated to 200 chars in text form, full in JSON) |
| source_session | `dec:sourceSession` (rendered as the verify-run's VG short id when the IRI matches the `activity/verify-graph-run/VG-NNN/...` pattern) |
| source_tc | `dec:sourceArtifact` (rendered as short TC id) |
| addressing_artifact | `dec:addressingArtifact` (rendered as VG-NNN or CC-NNN short id depending on class) |
| receiving_session | `dec:receivingSession` (the worker dispatch activity) |
| routed_at | `dec:routedAt` if present |

Sorted ascending by `created_at`. Two entries with identical timestamps are ordered by feedback IRI.

Text rendering is a compact two-line stanza per feedback (a header line with the state-coloured glyph plus IDs, and a `↳ evidence` continuation), with a final summary footer.

#### 2. `dec loop list`

```
dec loop list [--state open|closed|all] [--format text|json]
```

Default state is `open`. Groups all `dec:Feedback` artifacts by deriving the owning feature from `dec:sourceArtifact` (TC IRI → look up TC's `validates.features` in the product graph). Returns a row per feature with:

- `feature_id`
- `open_count` (lifecycle ∈ {produced, routed, received})
- `closed_count` (lifecycle ∈ {addressed, closed})
- `last_emitted_at` (max timestamp of any feedback in the group)

Sorted by `open_count DESC`, then by `last_emitted_at DESC`. The "what's noisiest right now" dashboard.

#### 3. Worker-dispatch session materialisation

`features::verify_graph_generate::run_generate` and `features::implement::run` are the two dispatch paths that today attach `activity/...` IRIs as `receivingSession` on feedback without registering them as `dec:Session`. This slice changes both to emit a proper `dec:Session` artifact at dispatch start, with:

- `rdf:type dec:Session`
- `prov:startedAtTime <ts>`
- `dec:roleId "verify-graph-author"` / `"implementer"`
- `dec:status "in-progress"` (transitioned to `"completed"` or `"failed"` at dispatch end)
- `dec:featureId "FT-NNN"` for the dispatch's target feature
- `dec:inStream <stream IRI>`

This is what `dec session list` and `dec session log` already query against — once the artifacts exist, the existing surfaces light up. No CLI changes for session inspection.

### Outputs

Both views render text by default (operator-friendly columns) and JSON on `--format json` for piping into downstream tools.

`dec loop show` exits 0 when feedback is found, 0 with `(no feedback for FT-NNN)` when none, and non-zero only on store-read failure.

`dec loop list` exits 0 always (an empty rollup is a legitimate state).

### State

- No on-disk schema change.
- Reads: feedback artifacts (via the existing `core::feedback::read` API), session artifacts, and the product graph for TC → feature reverse-lookup.
- Writes: worker-dispatch session artifacts on each `run_generate` and `implement::run`.

### Behaviour

1. **`dec loop show`** runs against the orchestration store + the product graph. The product graph's TC → feature edges populate the feature-scope filter; the orchestration store's feedback artifacts populate the rendered chain.
2. **`dec loop list`** runs against the same two stores but inverts the lookup: every defect feedback → owning feature → grouped count.
3. **Worker-session materialisation** happens inline in the dispatch path. A failed worker invocation transitions the session to `"failed"` instead of `"completed"` so audit can distinguish completed-but-rejected (worker did its job, FT-102 validator refused) from worker-side errors (subprocess crash, model timeout).

### Error handling

- Unresolvable TC → silently dropped from the rollup (logged at trace level). Feedback whose `sourceArtifact` isn't a TC (e.g. catalog-gap feedback pointing at a CapabilityReference) is excluded from `dec loop list`'s feature rollup but still listed in a final "(N feedback artifacts not scoped to any feature)" line.
- Missing product graph (running outside an initialised tree) → `dec loop list` errors with a clear remediation hint ("run inside a `dec init`-ed working directory"). `dec loop show` errors the same way.

### Out of scope

- Metrics roll-up: time-to-close per loop, average worker cost per addressed defect, worker-quality scoreboard. The data exists; a metrics CLI is a follow-up.
- Real-time tailing of in-progress loops. `dec events tail` already streams the events; the loop reporter is a post-hoc summary.
- Visualising the chain as a DAG (Graphviz / Mermaid). The text/JSON output is intentionally line-oriented for terminal use.

## Acceptance

1. `dec loop show <FT-NNN>` prints exactly one entry per defect feedback whose `sourceArtifact` is in FT-NNN's TC set, chronologically sorted, with state and addressing-artifact resolved.
2. `dec loop list` returns a row per feature with correct open/closed counts and is sorted by `open_count DESC`. The boundary case of an "all closed" feature appears in `--state all` and `--state closed` but NOT in the default `--state open`.
3. After running `dec verify graph generate FT-X --env ENV --accept` (worker dispatch), `dec session list` includes a row whose IRI matches the dispatch's `receivingSession` IRI and whose role is `verify-graph-author`. Equivalent for `dec implement FT-X`.
4. `dec loop show` correctly resolves `source_session` IRIs of the form `activity/verify-graph-run/VG-NNN/...` to the matching VG short id, and `addressing_artifact` IRIs of the form `graph/VG-NNN` and `code-change/CC-NNN` to their short ids.

## Notes

The reporter is small (~250 lines + tests) but operationally significant — it's what turns the FT-107/108 loop machinery from "trust me, the chain is there" into "here it is, in one screen". After this lands, the dogfood story for the verify → re-fix loop is complete from a UX standpoint, not just a data standpoint.
