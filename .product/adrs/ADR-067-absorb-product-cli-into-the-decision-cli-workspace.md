---
id: ADR-067
title: Absorb product-cli into the decision-cli workspace
status: superseded
features:
- FT-105
- FT-106
- FT-136
supersedes: []
superseded-by:
- ADR-077
domains: []
scope: domain
content-hash: sha256:62e9bdb5fccca042d0b012f50368c1f7f11378128bf6b87b5885e794ab078dd4
---

## Context

[ADR-009](ADR-009) scoped product-cli integration to *"subprocess CLI for reads, MCP write tools for writes"* and was explicit that this was a **slice-1 stance**. The reasoning at the time was sound: slice 1 needed the loose coupling to keep product-cli oblivious to decision-cli, and the service-based integration realised the *"single interface for humans and LLMs"* principle. Slice 1 has long since shipped; we are deep into slice 3.

Three pressures have accumulated since:

1. **Cross-repo specs are stacking up.** Recent slices keep authoring features in decision-cli's graph that target product-cli changes: [FT-076](FT-076) (Brief artifact in product-cli's catalog), [FT-104](FT-104) (default-acknowledge mechanism), the in-flight platform-scope work tied to [FT-103](FT-103)'s cleanup. Each one is a spec authored here, implementation there, and the spec has to track upstream evolution manually. The cross-repo coordination surface is growing roughly linearly with each new slice.

2. **product-cli evolution is decision-cli-driven.** The bundle-completeness diagnosis ([ADR-066](ADR-066)) required reading both codebases together (the user's source-read of `src/domains/preflight.rs` in product-cli to diagnose a decision-cli preflight noise problem). Every recent product-cli change has had a decision-cli motivation. The "product-cli evolves independently" outcome ADR-009 anticipated has not materialised — and that's fine, because the *useful* product-cli features for decision-cli's roadmap are what's driving the work.

3. **The existing plan named the absorbed end state.** `CLAUDE.md §CLI vocabulary` lists `dec product <subcommand>` in the slice-3+ vocabulary, with the note *"folds in once product-cli is absorbed into the workspace."* The slice-1 boundary in ADR-009 was always meant to be temporary; this ADR records the moment it is dissolved.

The structural question now is not *whether* to absorb but *how to preserve* the right invariants while doing so. The wrong absorption would re-create the smells ADR-009 prevented: product-cli accidentally importing DDD concepts, release cycles coupled in destructive ways, the orchestrator's CLI calls no longer reproducible by a human.

## Decision

**product-cli is absorbed into the decision-cli Cargo workspace as a sibling crate `crates/product-cli/`. The two-repo split is dissolved.** The absorption preserves the SDP boundaries ADR-009 protected: product-cli depends on nothing in decision-cli; decision-cli depends on product-cli; their crate boundary is the chokepoint. The integration mode shifts from subprocess to direct Rust API for the hot path; subprocess invocation remains supported for human-reproducibility; MCP stays for LLM tool-use.

Concretely:

### Workspace layout

```
.
├── crates/
│   ├── oxi-events/           (existing — event substrate, depends on nothing in dec/product)
│   ├── product-cli/          (NEW — absorbed via git subtree; depends on nothing in dec)
│   └── decision-cli/         (existing — depends on oxi-events AND product-cli)
├── workers/                  (existing Python workers, unchanged)
└── ...
```

product-cli is a workspace member at the **crate level**, like oxi-events. The SDP direction is: `decision-cli → product-cli → (nothing)`. Adding `decision-cli` to product-cli's dependency graph is a compile-time error, enforced the same way [ADR-016](ADR-016) enforces decision-cli → oxi-events. This is the single architectural rule that keeps the absorption from turning into accidental coupling.

### Integration mode shift

- **Reads from product-cli's graph state** (e.g. `feature_show`, `context`, `preflight`, `graph_check`) — go through product-cli's **public Rust API**, not subprocess. Cheaper, faster, no stdout-parsing fragility, and the function signatures become the contract.
- **CLI invocations from a human** (e.g. `dec product feature show FT-097`) — go through the same Rust API, just wrapped by the dec binary's command tree. No subprocess.
- **MCP tool-use** (the verify-graph-author worker calling `dec_verify_graph_generate`) — continues to be MCP, unchanged. MCP is the LLM interface; the absorption does not affect it.
- **Subprocess `product <verb>` invocations** — supported via a thin shim binary for backwards compatibility during the migration period, deprecated and removed in a later slice. The shim just delegates to the Rust API, exits with the same code, prints to the same streams.

The *interface principle* from ADR-009 (*"a single interface for humans and LLMs"*) is preserved: the dec binary surfaces both human verbs (`dec product feature show ...`) and MCP tools, both backed by the same Rust API. What changes is that the **transport** between dec and product-cli is no longer a process boundary.

### CLI verb mapping

Every product-cli verb gains a `dec product *` form: `product feature show FT-097` → `dec product feature show FT-097`. The argument syntax is identical; the dec binary's clap tree re-uses product-cli's clap definitions (re-export, not copy). The standalone `product` binary continues to exist for the deprecation period, invoking the same code via the shim.

### MCP server

The two separate MCP servers (`product` MCP exposing artifact-management tools, `dec` MCP exposing orchestration tools per [FT-034](FT-034)) merge into **one combined MCP server** exposed by the dec binary. Tool names stay distinct (no rename); the combined server exposes the union of both sets. This reduces operational overhead — one MCP process per project, not two — and matches the single-CLI-binary principle.

### Storage and graph state

The two on-disk locations remain distinct: `.product/` (product-cli's artifact tree, the human-authored canonical state) and `.dec/` (orchestration store, sessions, verification graphs, results). They are conceptually different layers and stay so. *Internally*, both may be projected into a **single oxigraph store** with two named graphs (`<product>` and `<orchestration>`) so cross-layer SPARQL queries become possible without merging the on-disk representations. The named-graphs choice is consistent with the [ADR-002](ADR-002) graph-as-state stance and does not require schema migration; existing readers continue to see their own graph.

### Git history

The absorption uses `git subtree add --prefix=crates/product-cli https://github.com/Hafeok/product-cli main` so product-cli's commit history is preserved verbatim under `crates/product-cli/`. Tags from the old repo are reachable; `git log crates/product-cli/` shows the full history. This matters because the cross-repo specs ([FT-076](FT-076), [FT-104](FT-104)) can be implemented in single commits that touch both sides of the (former) boundary — and `git blame` keeps working.

### Standalone product-cli repository

The github.com/Hafeok/product-cli repository becomes a **read-only archive** with a README pointing at the merged workspace. No further development happens there. Any community PRs are redirected here. (This is the cost the user accepted in the prior conversation; if standalone product-cli adoption matters more than expected, the cost re-evaluates.)

## Rejected alternatives

### Keep separate repos, invest in cross-repo workflow tooling

Lockstep release tags, shared CI templates, an automation bot that opens paired PRs across repos. Rejected — the engineering investment is large, the friction reduction is partial (the two-edit ceremony for any cross-cutting change remains), and the design intent in CLAUDE.md was already to absorb. The work is better spent on the absorption itself.

### Library integration only (no workspace merge)

decision-cli depends on product-cli as a crates.io dependency, both repos stay separate but the integration shifts to direct Rust API. Rejected — this gets the integration win without the source-of-truth win. We'd still author specs in one repo for code in another; debugging would still cross repos; cargo-version-bumping ceremony for every product-cli change would be friction. The workspace merge is what eliminates the cross-repo spec mode.

### Full module merge (one crate, not two)

Merge product-cli's source into `crates/decision-cli/src/product/`. Rejected — destroys the SDP boundary that ADR-009's separate-repos arrangement protected. With one crate, nothing prevents product-cli code from importing from `crate::orchestration::*`. The crate boundary is load-bearing; preserve it.

### Defer until later

Wait until slice 4 or 5. Rejected — the friction compounds. Doing it now (with FT-098/FT-099/FT-101/FT-102 still in spec) means those implementations can assume the absorbed shape from the start. Doing it later means migrating in-flight code plus the spec change.

### Reverse the absorption (decision-cli into product-cli)

Conceptually possible but architecturally wrong — product-cli is *below* decision-cli in the DDD stack. The orchestrator depends on the artifact catalog; not the other way around. Putting decision-cli inside product-cli would violate the dependency direction CLAUDE.md and ADR-009 both establish.

## Consequences

### Positive

- **Cross-repo spec mode ends.** Features like [FT-076](FT-076), [FT-104](FT-104), the platform-scope work, and any future product-cli evolution become intra-repo specs implemented in a single PR.
- **Direct Rust API for hot-path reads.** No subprocess overhead for `feature_show`, `preflight`, `context`. Latency drops measurably; stdout parsing fragility disappears.
- **One CI pipeline, one release artifact.** Build, test, and ship as one workspace. `cargo test --workspace` exercises both halves together.
- **One MCP process per project.** Operationally simpler — one stdio/http endpoint for LLM tool-use.
- **Git history preserved.** `git log crates/product-cli/` works; the past is intact.
- **CLAUDE.md's `dec product *` plan is now actionable.** The slice-3+ vocabulary becomes the reality.
- **SDP boundary is structurally enforced.** product-cli still cannot import decision-cli; the absorption does not regress the architectural separation, it just collocates the source.

### Negative / accepted trade-offs

- **product-cli standalone identity dissolves.** Anyone considering it for a non-DDD use case will find an archived repo pointing at decision-cli's workspace. If standalone adoption was an active goal, this is a real cost; if it was hypothetical, the cost is theoretical and the friction reduction is concrete.
- **Migration effort.** Git subtree merge, Cargo.toml updates, CLI re-export wiring, MCP merge, deprecation shim for the standalone binary, test suite consolidation. Bounded but non-trivial; [FT-105](FT-105) scopes the work.
- **Larger workspace build.** `cargo build --workspace` now compiles product-cli alongside decision-cli. Build time grows linearly; mitigated by incremental compilation.
- **The shim for the standalone `product` binary** carries deprecation cost. It needs a removal date and a clear migration message. [FT-105](FT-105) sets the deprecation window.
- **Pre-absorption product-cli releases become orphans.** Tagged versions on the old crates.io listing (if any) are frozen at absorption time; future versions are not re-published to the old name.

### Relationship to [ADR-009](ADR-009)

ADR-009 is **partially superseded** by this ADR:

- **Superseded:** the integration mode (subprocess + MCP) for the *hot path*. After absorption, hot-path reads use direct Rust API; subprocess invocation is supported for human-reproducibility and the standalone shim, not as the primary mode.
- **Preserved:** the *principle* that "a single interface for humans and LLMs" is the right shape. The dec binary surfaces both human verbs and MCP tools, both backed by the same Rust API — the principle is intact, the transport changed.
- **Preserved:** product-cli's obliviousness to DDD concepts. The SDP boundary at the crate level is the chokepoint that keeps product-cli a generic artifact catalog rather than a DDD orchestrator's appendage.

The ADR-009 record stays in the catalog with its slice-1 scoping intact; this ADR notes the partial supersession. Operators reading ADR-009 should be directed forward to ADR-067 for the slice-3+ stance.

## Status

Proposed. Bound to slice 3. [FT-105](FT-105) carries the implementation slice; once that lands and `dec product *` is verifiable against the absorbed shape, this ADR advances to accepted via the standard supersede ceremony.
