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
│   ├── oxi-events/                # Graph-native event substrate. SDP boundary at the crate
│   │                              # level: depends only on oxigraph, tokio, tokio-stream,
│   │                              # axum, serde, tracing. NEVER imports from decision-cli
│   │                              # or references DDD concepts.
│   └── decision-cli/              # The orchestration crate (binary name `dec`).
│       └── src/
│           ├── core/              # SDP boundary at the slice level: stable substrate
│           │                      # within decision-cli. Depended on by all features.
│           │                      # NEVER imports from features::*.
│           │   ├── graph/         # Oxigraph wrapper, named graph management
│           │   ├── ontology/      # base types (Session, Goal, Dispatch)
│           │   ├── bundle/        # SPARQL CONSTRUCT execution
│           │   ├── harness/       # generic dispatch loop
│           │   └── observability/ # tracing, error types
│           ├── features/          # Vertical slices, one per feature_spec.
│           │   ├── ft_001_init/   # dec init, end-to-end (command, validation, query, tests)
│           │   ├── ft_002_status/
│           │   ├── ft_003_implement/
│           │   └── ...            # NEVER import from other features::*
│           └── main.rs            # Wiring only. No business logic.
├── workers/
│   └── code-writer/               # Python worker for the implementer role. Stateless:
│                                  # receives bundles, calls Claude, returns CodeChange.
├── docs/
│   ├── ddd/                       # DDD foundational docs (read-only reference).
│   ├── features/                  # product-cli features for decision-cli's engineering work.
│   ├── adrs/                      # product-cli ADRs.
│   ├── tests/                     # product-cli test criteria.
│   └── product.toml               # product-cli configuration for this repo.
├── streams/
│   └── decision-cli-development.ttl  # ValueStream definition for `dec init --from`
├── decision-cli-slice-1-bounds.md    # Current scope and boundaries.
└── README.md
```

## The principle that governs everything

**Engineering work for decision-cli is authored through product-cli first.** When asked to add a feature, change behavior, or make a design decision, the first move is the `product` CLI — not editing files directly.

- New work → `product feature new` to author a feature_spec.
- New decision → `product adr new` to author an ADR.
- New verification criterion → `product tc new`.
- Browse current state → `product feature list`, `product graph stats`, `product context FT-XXX`.

Direct file edits in `crates/` and `workers/` should happen only as the *implementation* of an existing feature_spec, after the spec has been authored in product-cli and any required ADRs are in place. If you find yourself wanting to make a structural change with no corresponding feature_spec or ADR, stop and author one first.

## Definition of done (read this before touching code)

**A feature is complete if and only if `product verify FT-XXX` exits 0.** Nothing else counts. Not "cargo test passes," not "I wrote the code and a test," not `product feature status FT-XXX complete` — that command only flips a status field; it does not certify anything. The verify pipeline does.

`product verify FT-XXX` walks every TC linked to FT-XXX, executes each TC's configured runner, and only succeeds when all of them pass. So "make verify pass" decomposes into one obligation per linked TC: **the test the TC points to must exist, must be discoverable by the declared runner, and must pass.**

### The implementation lifecycle (the flow that must be completed every time)

1. **Pick** — `product feature next` returns the next feature whose dependencies are satisfied. Do not skip the dependency order.
2. **Plan** — Read the spec and what governs it:
   ```bash
   product feature show FT-XXX        # spec, linked ADRs, linked TCs
   product context FT-XXX --depth 2   # the bundle the implementer reads
   product preflight FT-XXX           # domain & cross-cutting coverage
   product gap check FT-XXX           # spec gaps that would block work
   ```
   If preflight or gap surfaces something missing, fix the upstream artifact first (author an ADR, extend a feature) — do not paper over it in code.
3. **Implement** — Write the feature code in the slice (`crates/decision-cli/src/features/ft_NNN_*/` or the worker directory) following the SDP rules below.
4. **Wire the TC runners** *(this is the step that gets skipped — do not skip it)*. For every TC reported by `product feature show FT-XXX`:
   1. `product test show TC-YYY` — read the acceptance criteria and the current `runner` / `runner-args` / `runner-timeout` fields in the frontmatter.
   2. Write the test the TC describes (`#[test]` in the right crate, `pytest` in the worker, or a bash script under `tests/scripts/`). The test must produce a binary pass/fail via its exit code; the product-cli runner contract is two-tier (0 = pass, 1 = fail; anything else is `unrunnable`, per ADR-013).
   3. Set or update the runner so it actually points at what you wrote:
      ```bash
      product test runner TC-YYY --runner cargo-test --args "<test name>" --timeout 120s
      product test runner TC-YYY --runner bash       --args "tests/scripts/tc-yyy-foo.sh" --timeout 60s
      product test runner TC-YYY --runner pytest     --args "workers/code-writer/tests/test_tc_yyy.py::test_x" --timeout 60s
      ```
      If the TC was pre-seeded with a `runner-args` value (most are), either honour that name when writing the test, or update it here to match what you actually wrote. **The TC frontmatter and your test file must agree — verify cannot guess.**
   4. Run that single TC end-to-end via its runner (e.g. `cargo test -p <crate> --test <name>`, `bash tests/scripts/...`, `pytest <path>`) and watch it pass before moving on.
5. **Verify the whole feature** — `product verify FT-XXX`. If any linked TC is `unimplemented` or fails, you are not done; loop back to step 4 for that TC. The feature is only complete when this command exits 0.
6. **Commit** — Reference the feature in the message (see Commit messages below). `dec implement` (or the headless `product implement` path) handles commit + status flip automatically on success; for manual runs, commit the working tree and let `product implement` / `product verify` do the status update — do not hand-edit feature status to `complete` to make a green dashboard out of a red verify.

### Common failure modes in headless `implement-all` runs

- **Test written, runner not updated.** `cargo test` passes locally, but the TC's `runner-args` still references the pre-seeded name from when the TC was authored. `product verify` invokes the wrong target, fails. Fix: always run `product test runner TC-YYY --args ...` after writing the test, even if it feels redundant.
- **Status flipped without verify.** Calling `product feature status FT-XXX complete` does not run any tests. Do not call it as a shortcut to "finish" a feature. Let `product verify` decide.
- **Runner pointing at a missing path.** A `bash` runner with `runner-args: tests/scripts/tc-yyy.sh` will report `unrunnable` if that script doesn't exist. Always create the script (and `chmod +x` it) before updating the runner.
- **`product verify FT-XXX` skipped entirely.** If the loop exits "successfully" without `verify` having run and returned exit 0 for this feature, the feature is not done — regardless of what the session log says. Run verify explicitly.

## The line that must not be crossed (crate-level SDP)

`crates/oxi-events/` cannot depend on `crates/decision-cli/`. It cannot reference DDD concepts (roles, bundles, sessions, policies, model bindings, autonomy levels). Its public API speaks only of mutations, subscriptions, events, and delivery. This is the Stable Dependency Principle at the crate boundary — see [`decision-cli-slice-1-bounds.md`](decision-cli-slice-1-bounds.md) §5.1.

If a feature_spec asks for something in oxi-events that requires DDD vocabulary, that's a smell. The feature belongs in decision-cli, with oxi-events providing only the generic substrate it needs.

## Discipline within decision-cli (slice-level SDP)

decision-cli follows vertical slice architecture with SDP applied inside the crate as well. Two boundaries here are as load-bearing as the oxi-events boundary above:

**`core/` is depended on; never depends on `features/`.** Pure substrate. Graph access, base ontology types, the bundle assembly framework, the harness dispatch loop, observability scaffolding. When you find yourself wanting to import from `features::*` inside `core/`, that's the smell — the abstraction belongs in core or the feature needs different shape. The compile fails if you try; module visibility enforces this structurally.

**Features depend on `core/`; never on other features.** Each `features/ft_NNN_<title>/` directory is self-contained: command handler, feature-specific SPARQL, feature-specific validation, feature-specific tests. The 1:1 mapping with product-cli feature_specs is the convention — when product-cli says implement FT-007, the code goes in `features/ft_007_<title>/`. Cross-feature dependencies are not allowed; if two features need shared logic, that logic lives in `core/`.

**The binary (`main.rs`) is wiring only.** It composes feature handlers and routes CLI invocations. No business logic in main.rs; if you're tempted, the logic belongs in a feature or core.

**When patterns recur across features, migrate to core.** Vertical slice tolerates some redundancy in exchange for slice independence. When two slices grow similar code, that pattern is a candidate for `core` — author a feature_spec for the core extension, then individual slices adopt it. Migrations are themselves feature_specs, not silent refactors.

**Cross-cutting changes (new artifact types, new edge predicates) extend `core` first.** The slice introducing the change authors a feature_spec for the core extension; subsequent slices adopt it as needed. No silent cross-cutting through feature/feature edges.

## Common tasks

### Building everything

```bash
cargo build --workspace
cd workers/code-writer && uv sync   # or your Python tool of choice
```

### Running tests

```bash
cargo test --workspace                # raw Rust tests
cd workers/code-writer && pytest      # raw worker tests
product verify FT-XXX                 # the only completion signal for a feature — runs every linked TC's runner
product verify                        # full six-stage pipeline (FT-044) across the repo
product verify --platform             # cross-cutting / fitness TCs only (ADR-013, ADR-014)
```

`cargo test` and `pytest` are debugging aids while you iterate. `product verify FT-XXX` is the gate — see "Definition of done" above.

### Authoring an artifact in product-cli

```bash
product feature new FT-007 --title "Subscription registry"
# (then iterate via product author mode)
```

### Running the orchestrator end-to-end (slice 1 scope)

```bash
# First-time setup: create orchestration store, seed v0 subscriptions
dec init --from ./streams/decision-cli-development.ttl

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

## Rules live in `.product/`

**Code-quality rules, architectural constraints, and other fitness functions are ADRs with `scope: cross-cutting`** (per ADR-014). The `.product/` graph is the single source of truth; CI is the enforcer; the context bundle is the carrier.

Why this matters: Every implementation session in decision-cli receives a context bundle assembled from `.product/features/`, `.product/adrs/`, and `.product/tests/`. Cross-cutting ADRs surface automatically in every bundle. Rules in the graph means rules reach the implementer; rules in CONTRIBUTING.md or tribal knowledge do not.

### The structure

A rule is an ADR + TC pair:

- **ADR** (`.product/adrs/`) — carries `scope: cross-cutting`, declares the rule in its body (thresholds, rationale, script paths).
- **TC** (`.product/tests/`) — links back to the ADR via `validates.adrs: [ADR-XXX]`, points at the enforcement script via `runner` + `runner-args`.

Examples in this repo: ADR-013 (code structure limits — source file length, function length, single-responsibility comments), ADR-008 (worker contract), ADR-001 (SDP boundary on `oxi-events`).

### Lifecycle

**Add a rule.** The flow from decision to enforcement:

1. Author the ADR in `product author` mode (or via `product request apply` if you have a written request):
   ```bash
   product author adr          # or product adr new ADR-XXX --title "..."
   # In the session: describe the rule, set scope: cross-cutting, set domains: [...]
   ```
2. Write the enforcement script under `scripts/checks/`:
   ```bash
   # Example: scripts/checks/source-file-length.sh
   # Exit 0 if passes, 1 if fails, 2 if warnings
   ```
3. Author one or more TCs that link to the ADR:
   ```bash
   product test new TC-XXX --title "Source files under 400 lines"
   product test runner TC-XXX --runner bash --args "scripts/checks/source-file-length.sh" --timeout 30s
   # Set validates.adrs: [ADR-013] in the TC frontmatter
   ```
4. Apply the request (if using request flow):
   ```bash
   product request apply <request-id>
   ```
5. CI on the PR validates `product graph check` and `product verify --platform`.

**Change a rule.** ADRs go through the accepted-ADR amend flow:

```bash
product author adr          # or product adr amend ADR-XXX
# In the session: describe the change, record the reason
```

The amendment is recorded with a reason and a previous-hash; the request log carries the audit trail.

**Retire a rule.** Mark the ADR as `superseded` or `abandoned`:

```bash
product adr status ADR-XXX superseded --by ADR-YYY
# or
product adr status ADR-XXX abandoned
```

Cross-cutting TCs lose their parent rule and surface in `product graph check` until they are deleted or relinked.

### Enforcement via `product verify --platform`

A pull request CI step runs `product verify --platform`. This command executes every TC linked to a cross-cutting ADR. The exit code is the gate:

- `0` — every cross-cutting TC passes. Merge.
- `1` — at least one cross-cutting TC fails. Block.
- `2` — warnings only (e.g. a file is in the 300–400 line warning zone). Allow merge, surface in the PR comment.

There is no separate "linting" CI pipeline or fitness-functions config. All cross-cutting checks live in `.product/` and run through `product verify --platform`.

### Worked example: landing a new "no TODO comments without issue links" rule

From decision to automated enforcement:

```bash
# 1. Author the rule
product author adr
# In the session:
#   - Title: "TODO comments must link to an issue"
#   - Scope: cross-cutting
#   - Domains: [observability]
#   - Body: "Every TODO/FIXME comment must include a GitHub issue link (github.com/.../issues/NNN).
#            Orphaned TODOs accumulate and rot. Linked TODOs trace back to a reason and a plan."
# (session completes, writes .product/adrs/ADR-021-todo-comments-must-link.md and a request to requests.jsonl)

# 2. Write the enforcement script
cat > scripts/checks/todo-comments-have-links.sh <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
rg --type rust --type python 'TODO|FIXME' -n \
  | grep -v 'github\.com/.*/issues/[0-9]' \
  && { echo "Found TODO/FIXME without issue link"; exit 1; } \
  || exit 0
EOF
chmod +x scripts/checks/todo-comments-have-links.sh

# 3. Author the TC
product test new TC-042 --title "TODO comments link to issues"
product test runner TC-042 --runner bash --args "scripts/checks/todo-comments-have-links.sh" --timeout 30s
# Edit .product/tests/TC-042-todo-comments-link-to-issues.md frontmatter:
#   validates:
#     adrs: [ADR-021]
#     features: []

# 4. Apply the request
product request apply <request-id from step 1>

# 5. Verify locally
product verify --platform
# TC-042 runs scripts/checks/todo-comments-have-links.sh
# Exit 0 → rule passes

# 6. Commit and open PR
git add .product/adrs/ADR-021-*.md .product/tests/TC-042-*.md scripts/checks/todo-comments-have-links.sh .product/requests.jsonl
git commit -m "[ADR-021] Require issue links in TODO comments"
# CI on the PR runs `product verify --platform`, includes TC-042 in the fitness-function gate
```

From this point forward, every feature context bundle will include ADR-021, and every PR will run TC-042. The rule is in the system.

## CLI vocabulary

Slice 1 exposes a minimal subset of the `dec` command surface. The full vocabulary emerges over later slices and follows the single-command pattern of `az`/`gcloud`/`kubectl`:

- `dec drive <goal> <artifact>` — goal-driven dispatch; orchestrator plans the role chain to the value action.
- `dec dispatch role <role> <artifact>` — manual single-role dispatch (power-user escape, debugging, replay).
- `dec watch <role> [args]` — standing role (continuous observers, e.g., monitors).
- `dec schedule <role> --interval <duration>` — periodic role (meta-loop work).
- `dec product <subcommand>` — engineering artifact authoring (folds in once product-cli is absorbed into the workspace).
- `dec events`, `dec session`, `dec goal`, `dec role`, `dec model`, `dec policy`, `dec subscription`, `dec checkpoint` — inspection and management of graph entities.

Slice 1 implements only `dec init`, `dec status`, `dec implement`, `dec events`, `dec session`, and `dec health`. Later slices add the rest as the corresponding architectural pieces land (interpretation pairing, feedback flow, policy artifacts, the meta-loop).

## Conventions

### Rust

- Edition 2021. Format with `cargo fmt`. Lint with `cargo clippy --workspace -- -D warnings`.
- Errors: `thiserror` for libraries (including `oxi-events` and `core/`), `anyhow` for binaries and features (`features/*`).
- Async: tokio. Tracing: the `tracing` crate.
- Public APIs in `oxi-events` are documented with rustdoc; private items optional.
- `core/` exposes minimal `pub` surface (use `pub(crate)` aggressively); features access only what core deliberately exports.
- Each feature directory has its own integration test file (`features/ft_NNN_*/tests.rs`); cross-feature integration tests live in `crates/decision-cli/tests/`.

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

Every feature_spec exits with at least one test criterion (TC). The TC's success criteria are the acceptance test, and the gate that decides "done" is `product verify FT-XXX` — see "Definition of done" near the top of this file for the full lifecycle and the runner-wiring step that headless runs keep skipping.

In short: a TC carries a `runner` + `runner-args` pair in its frontmatter. Writing a test is not enough — the runner has to point at it (`product test runner TC-YYY --runner ... --args ...`), and then `product verify FT-XXX` has to come back green. Failing or `unimplemented` TCs block release per fitness-function policy; flipping `feature status complete` by hand does not.

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