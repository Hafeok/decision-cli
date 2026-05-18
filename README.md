# decision-cli

> The orchestration system for Decision-Driven Design — drives [product-cli](https://github.com/Hafeok/product-cli) through the engineering process by dispatching LLM-backed roles, recording sessions, and routing artifacts via a graph-native event substrate. Installs as the `dec` binary.

## What this is

decision-cli implements the orchestration layer described in [Decision-Driven Design](docs/ddd/Decision-Driven_Design.md). Where [product-cli](https://github.com/Hafeok/product-cli) manages engineering artifacts (features, ADRs, test criteria) and assembles curated context bundles, decision-cli runs the process: it invokes agents, records what they did, and (eventually) improves itself over time based on what worked.

The name embodies the framework's primary claim: **decisions are the unit of work**. Every command the binary exposes is an instance of making or inspecting a decision about the decision graph.

The architecture in one line: **the graph is the state, mutations are events, and roles are filled by models bound through policy.**

## Components

- **[`crates/oxi-events`](crates/oxi-events)** — graph-native event substrate for Oxigraph. Stable Dependency Principle: depends only on substrates more stable than itself (oxigraph, tokio, axum, serde, tracing). Intended for community contribution; dual-licensed Apache-2.0 OR MIT.
- **[`crates/decision-cli`](crates/decision-cli)** — the orchestration crate, installed as the `dec` binary. Depends on oxi-events and on product-cli (via subprocess invocation in slice 1).
- **[`workers/code-writer`](workers/code-writer)** — Python worker for the implementer role. Calls Claude via structured output, returns typed `CodeChange` artifacts.
- **[`docs/`](docs)** — product-cli graph for decision-cli's own engineering work, plus the DDD reference documents.

## Status

Slice 1 in design. See [`decision-cli-slice-1-bounds.md`](decision-cli-slice-1-bounds.md) for the operational scope — what's in slice 1, what's deliberately deferred, and how to start.

## Architecture documents

| Document | Purpose |
|---|---|
| [`docs/ddd/Decision-Driven_Design.md`](docs/ddd/Decision-Driven_Design.md) | The framework: decisions as the unit of work, artifacts as the unit of composition. |
| [`docs/ddd/Decision-Driven_Design__Entity_Reference.md`](docs/ddd/Decision-Driven_Design__Entity_Reference.md) | Vocabulary reference for the framework's entities. |
| [`docs/ddd/DDD_and_the_Five_Levels_of_AI_Autonomy.md`](docs/ddd/DDD_and_the_Five_Levels_of_AI_Autonomy.md) | How DDD maps to per-role autonomy graduation. |
| [`docs/ddd/Implementing_DDD.md`](docs/ddd/Implementing_DDD.md) | The implementation architecture (tech choices, patterns, decisions). |
| [`decision-cli-slice-1-bounds.md`](decision-cli-slice-1-bounds.md) | Current scope and boundaries. |

If you're working in this repo with Claude or another LLM coding assistant, also read [`CLAUDE.md`](CLAUDE.md).

## Building

```bash
# Rust workspace
cargo build --workspace
cargo test --workspace

# Python worker
cd workers/code-writer && uv sync && pytest
```

## Running (slice 1)

```bash
# Author a feature in product-cli (initialized in docs/)
product feature new FT-007 --title "Subscription registry"

# Implement it via decision-cli
dec implement FT-007

# Watch events flow
dec events tail
```

## CLI vocabulary

decision-cli follows the single-command pattern of `az`, `gcloud`, `kubectl`. Every command is `dec <namespace> <verb> [args]`. The full vocabulary emerges over later slices:

```
# Goal-driven dispatch — orchestrator plans the role chain to a value action
dec drive ship FT-007

# Manual single-role dispatch — power-user escape
dec dispatch role implementer FT-007

# Standing role — always-on observer
dec watch monitor --env production

# Periodic role — meta-loop work
dec schedule pattern-detector --interval 1h

# Engineering artifact authoring (eventually folds in from product-cli)
dec product feature new FT-007

# Inspection
dec events tail
dec session show ses-abc123
dec goal show FT-007
dec role list
dec policy show
dec checkpoint pending
```

Slice 1 implements the minimum: `dec init`, `dec implement`, `dec events`, `dec session`, `dec health`. Later slices add the rest.

## License

- `oxi-events` — Apache-2.0 OR MIT (dual-licensed, matching Oxigraph's convention).
- Everything else — Apache-2.0.

See [`LICENSE-APACHE`](LICENSE-APACHE) and [`LICENSE-MIT`](LICENSE-MIT).
