# CLAUDE.md — Instructions for Claude working in decision-cli

This file orients Claude (Claude Code, Claude in product-cli `author` mode, or any other instance) to this repository. Read it before making changes.

## What this project is

decision-cli is the orchestration system for Decision-Driven Design. It drives [product-cli](https://github.com/Hafeok/product-cli) — which manages engineering artifacts (features, ADRs, test criteria) and assembles curated context bundles — by dispatching LLM-backed role sessions, recording what each session produced, and routing artifacts between roles via a graph-native event substrate.

The binary is installed as `dec`. Documentation refers to "decision-cli" (the project); users type `dec` (the command).

For the full architectural picture, read [`docs/ddd/Implementing_DDD.md`](docs/ddd/Implementing_DDD.md). For the current scope of work, read [`decision-cli-slice-1-bounds.md`](decision-cli-slice-1-bounds.md).

## Where things live

```
.
├── crates/
│   ├── oxi-events/          # Graph-native event substrate. SDP boundary: depends only on
│   │                        # oxigraph, tokio, tokio-stream, axum, serde, tracing. NEVER
│   │                        # imports from decision-cli or references DDD concepts.
│   └── decision-cli/        # The orchestration crate (binary name `dec`).
│                            # Depends on oxi-events.
├── workers/
│   └── code-writer/         # Python worker for the implementer role. Stateless: receives
│                            # bundles, calls Claude, returns structured CodeChange artifacts.
├── docs/
│   ├── ddd/                 # DDD foundational docs (read-only reference).
│   ├── features/            # product-cli features for decision-cli's own engineering work.
│   ├── adrs/                # product-cli ADRs.
│   ├── tests/               # product-cli test criteria.
│   └── product.toml         # product-cli configuration for this repo.
├── decision-cli-slice-1-bounds.md   # Current scope and boundaries.
└── README.md
```

## The principle that governs everything

**Engineering work for decision-cli is authored through product-cli first.** When asked to add a feature, change behavior, or make a design decision, the first move is the `product` CLI — not editing files directly.

- New work → `product feature new` to author a feature_spec.
- New decision → `product adr new` to author an ADR.
- New verification criterion → `product tc new`.
- Browse current state → `product feature list`, `product graph stats`, `product context FT-XXX`.

Direct file edits in `crates/` and `workers/` should happen only as the *implementation* of an existing feature_spec, after the spec has been authored in product-cli and any required ADRs are in place. If you find yourself wanting to make a structural change with no corresponding feature_spec or ADR, stop and author one first.

## The line that must not be crossed

`crates/oxi-events/` cannot depend on `crates/decision-cli/`. It cannot reference DDD concepts (roles, bundles, sessions, policies, model bindings, autonomy levels). Its public API speaks only of mutations, subscriptions, events, and delivery. This is the Stable Dependency Principle — see [`decision-cli-slice-1-bounds.md`](decision-cli-slice-1-bounds.md) §4.1.

If a feature_spec asks for something in oxi-events that requires DDD vocabulary, that's a smell. The feature belongs in decision-cli, with oxi-events providing only the generic substrate it needs.

## Common tasks

### Building everything

```bash
cargo build --workspace
cd workers/code-writer && uv sync   # or your Python tool of choice
```

### Running tests

```bash
cargo test --workspace
cd workers/code-writer && pytest
```

### Authoring an artifact in product-cli

```bash
product feature new FT-007 --title "Subscription registry"
# (then iterate via product author mode)
```

### Running the orchestrator end-to-end (slice 1 scope)

```bash
# First-time setup: create orchestration store, seed v0 subscriptions
dec init

# Once a feature_spec exists and is ready for implementation:
dec implement FT-007
```

### Inspecting events and sessions

```bash
dec events tail              # subscribe to live events via SSE
dec events since 1234        # replay events from a sequence number
dec session list             # recent sessions
dec session show <id>        # session details with bundle hash and output ref
dec session log <id>         # full PROV-O chain
```

### Checking graph health

```bash
product graph check          # product-cli's existing audits
product preflight FT-007     # context coverage check for a specific feature
dec health                   # decision-cli liveness check
```

## CLI vocabulary

Slice 1 exposes a minimal subset of the `dec` command surface. The full vocabulary emerges over later slices and follows the single-command pattern of `az`/`gcloud`/`kubectl`:

- `dec drive <goal> <artifact>` — goal-driven dispatch; orchestrator plans the role chain to the value action.
- `dec dispatch role <role> <artifact>` — manual single-role dispatch (power-user escape, debugging, replay).
- `dec watch <role> [args]` — standing role (continuous observers, e.g., monitors).
- `dec schedule <role> --interval <duration>` — periodic role (meta-loop work).
- `dec product <subcommand>` — engineering artifact authoring (folds in once product-cli is absorbed into the workspace).
- `dec events`, `dec session`, `dec goal`, `dec role`, `dec model`, `dec policy`, `dec subscription`, `dec checkpoint` — inspection and management of graph entities.

Slice 1 implements only `dec init`, `dec implement`, `dec events`, `dec session`, and `dec health`. Later slices add the rest as the corresponding architectural pieces land (interpretation pairing, feedback flow, policy artifacts, the meta-loop).

## Conventions

### Rust

- Edition 2021. Format with `cargo fmt`. Lint with `cargo clippy --workspace -- -D warnings`.
- Errors: `thiserror` for libraries (including `oxi-events`), `anyhow` for binaries (`decision-cli`).
- Async: tokio. Tracing: the `tracing` crate.
- Public APIs in `oxi-events` are documented with rustdoc; private items optional.
- Tests live alongside the code (`#[cfg(test)] mod tests`) for unit tests; integration tests in each crate's `tests/` directory.

### Python (workers)

- Python 3.11+. Format with `ruff format`. Lint with `ruff check`.
- Pydantic for structured outputs. `anthropic` SDK for Claude calls.
- Workers are stateless: bundle in, artifact out. **No graph access from workers** — the harness owns reads and writes. If you find yourself wanting to query the graph from a worker, that's a contract violation; the harness should pass what the worker needs in the bundle.

### Commit messages

Reference the feature_spec or ADR being implemented:

```
[FT-007] Add Subscription registry
[ADR-003] Apply graph-as-state principle
```

Commits that don't trace to an artifact should be rare and explainable (typos, formatting, dependency bumps).

### Tests as exit criteria

Every feature_spec exits with at least one test criterion (TC). The TC's success criteria are the acceptance test. Failing TCs block release per fitness function policy. When implementing a feature, the TCs are the definition of done — not "the code compiles" or "I think it works."

## When something is unclear

If a feature_spec's context doesn't contain enough to act on confidently:

- **Authoring mode** (you're in `product author`): the spec needs more detail. Ask the human, add missing context, or write an ADR for the underlying decision before continuing.
- **Implementation mode** (filling the implementer role): emit feedback via the worker's `emit_feedback` mechanism with class `gap` or `contradiction`. The orchestrator will route it upstream and pause the current session. (Note: slice 1 does not yet have structured feedback; report the issue as a session error until slice 2 lands feedback flow.)

Do not improvise on architectural decisions. Architectural choices belong in ADRs, not commit messages or in-line comments.

## Cross-references

- [`docs/ddd/Decision-Driven_Design.md`](docs/ddd/Decision-Driven_Design.md) — the framework.
- [`docs/ddd/Decision-Driven_Design__Entity_Reference.md`](docs/ddd/Decision-Driven_Design__Entity_Reference.md) — vocabulary.
- [`docs/ddd/DDD_and_the_Five_Levels_of_AI_Autonomy.md`](docs/ddd/DDD_and_the_Five_Levels_of_AI_Autonomy.md) — autonomy levels and graduation.
- [`docs/ddd/Implementing_DDD.md`](docs/ddd/Implementing_DDD.md) — implementation architecture (primary reference).
- [`decision-cli-slice-1-bounds.md`](decision-cli-slice-1-bounds.md) — what slice 1 is and is not.
- [product-cli](https://github.com/Hafeok/product-cli) — the engineering process system.
