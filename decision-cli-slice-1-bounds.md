# decision-cli — Slice 1 Bounds Document

**Status:** Draft
**Version:** 0.3
**Companion:** Structured specification authored in product-cli (`docs/features/`, `docs/adrs/`, `docs/tests/`).

---

## 1. What this document is

This is the architectural narrative for the first implementation slice of **decision-cli** (binary name `dec`), the orchestration system that drives **product-cli** through the engineering process. It defines what slice 1 is, what it deliberately is not, and the principles that govern the design.

The structured specification — feature_specs, ADRs, test criteria — is authored in product-cli's graph and is the operational source of truth. This document is the frame; product-cli's graph is the specification.

The reader should come away knowing what is being built, why it is bounded this way, and where the line is between decision-cli and product-cli.

---

## 2. Premise and inheritance

decision-cli is the Decision-Driven Design implementation of the orchestration system. Where product-cli is the system implementation for the engineering process (it manages features, ADRs, test criteria, dependencies, and the curated context bundles that drive role decisions), decision-cli is the system that *runs the process* — dispatching LLM-backed roles, recording sessions, routing artifacts between roles, and improving its own behavior over time.

The name embodies the framework's primary claim: decisions are the unit of work. Every command the binary exposes is an instance of making or inspecting a decision about the decision graph. The short alias `dec` reduces typing overhead in daily use.

The DDD framing carries forward in full:

- **Decisions are the unit of work, artifacts are the unit of composition.** Every role consumes a deterministic bundle and produces a typed artifact. Roles never talk to each other; they produce and consume artifacts.
- **The graph is the system state.** All artifacts, all roles' outputs, all session records, and all events live in the graph. SPARQL is the query surface. Named graphs preserve provenance and history.
- **Single interface for humans and LLMs.** product-cli's existing MCP + CLI surface is the only way to read and mutate the engineering graph. decision-cli adds an orchestration surface on top that uses the same interface humans use.
- **Per-role model selection.** Each role is a context bundle; the model that fills the role is chosen for the bundle's shape and the decision's profile. This is a policy declaration, not a deployment detail.
- **The factory builds itself.** The first product decision-cli operates on is its own next slice. Slice 1 is built manually; slice 2 onward is the system processing its own feature_specs.

---

## 3. Value stream scoping

Each decision-cli instance is scoped to a single value stream. A value stream is the full chain of processes terminating in a specific value action. For slice 1, that stream is **decision-cli's own development**, with terminal value action **shipped-feature**.

This scoping is concrete, not nominal: the value stream and the value action it pursues are declared by reference to canonical, schema-validated definitions, not by raw strings on the command line. At `dec init` time the definitions are loaded, validated, and persisted as graph artifacts in the orchestration store. The instance's identity is then unambiguous: every Session, Goal, Dispatch, and downstream artifact carries a `dec:inStream` link back to the ValueStream artifact, and the orchestrator enforces the scope at command time.

### 3.1 Definitions, not strings

The original v0.1 sketch had `dec init` take two strings (`--stream` and `--value-action`). That shape is wrong: strings carry no schema, no validation, no provenance, and no reusability. Two instances both declaring "shipped-feature" via raw strings have no guarantee of meaning the same thing — exactly the drift the framework should prevent.

The fix is layered:

- **Base ontology** ships with decision-cli — declares `dec:ValueStream`, `dec:ValueAction`, `dec:Goal`, and the SHACL shapes that constrain their required fields. Versioned with the binary; embedded as a static asset.
- **ValueAction definitions** are first-class artifacts with stable URIs (e.g., `https://decision-cli.dev/ns/value-actions/shipped-feature`). Each definition document specifies name, description, exit criteria, expected output artifact types, compatible goals, and any constraints. Multiple streams referencing the same URI are guaranteed to mean the same thing — that's the entire point of having canonical definitions.
- **ValueStream definitions** are per-instance Turtle/JSON-LD documents that reference a ValueAction by URI and declare stream-specific config: name, title, description, the authorized goals (which must intersect the referenced ValueAction's compatible-goals set), and any base policy references.

### 3.2 The init contract

`dec init` takes a reference to a ValueStream definition, never raw strings:

```bash
# Bundled template — fast path for common cases
dec init --template engineering-development

# Custom definition — file path
dec init --from ./streams/decision-cli-development.ttl
```

`dec init` refuses to run without an explicit reference. Value stream identity is too load-bearing to default-guess.

A ValueStream definition document:

```turtle
@prefix dec: <https://decision-cli.dev/ns#> .
@prefix va:  <https://decision-cli.dev/ns/value-actions/> .

<stream:decision-cli-development> a dec:ValueStream ;
    dec:name                "decision-cli-development" ;
    dec:title               "Decision CLI Development" ;
    dec:description         "Development of the decision-cli orchestration system itself" ;
    dec:terminalValueAction va:shipped-feature ;
    dec:authorizedGoals     ( "ship" "land" ) ;
    dec:basePolicy          <policy:default-engineering> .
```

The `va:shipped-feature` URI resolves to a canonical, shared ValueAction definition — bundled with decision-cli in slice 1, fetched from a known registry in later slices. The definition itself carries exit criteria and constraints; every stream pursuing `va:shipped-feature` is provably aimed at the same thing.

### 3.3 Init validates and records provenance

Before any orchestration state is written, init performs:

1. **Parse** the referenced definition document (Turtle or JSON-LD).
2. **SHACL-validate** against the base ontology. Required fields missing, wrong types, or constraint violations → clear error, no state written.
3. **Resolve** the ValueAction URI. Slice 1 resolves only against the bundled set; unbundled URIs are an error.
4. **Cross-validate**: authorized goals must intersect the ValueAction's compatible-goals list. Missing or invalid → error.
5. **Persist** the validated ValueStream and ValueAction artifacts into the orchestration store, with content hashes and source provenance.
6. **Record** the bootstrap session (`dec:session/init-001`) with PROV-O links to: the definition source (path or template name), its content hash, the validation result, the base ontology version, and the timestamp.

After init, the instance's identity is graph-resident, schema-validated, and provenance-tracked. Inspecting `dec status` or running `dec session show init-001` makes the entire bootstrap auditable.

### 3.4 Enforcement at command time

`dec drive <goal> <artifact>` validates that the goal verb is in the stream's authorized list. `dec drive ship FT-007` succeeds in the `decision-cli-development` stream; `dec drive prioritize FT-007` is refused with a clear message ("This stream pursues `va:shipped-feature`; `prioritize` is not an authorized goal — try a stream with Discovery scope"). The boundary is structural — the orchestrator cannot drift outside its declared scope because the validation runs before any role is dispatched and the authorized-goals list is itself a property of the persisted ValueStream artifact, not a runtime flag.

### 3.5 Per-stream working directories

Like git, each value stream lives in its own working directory with a `.dec/` config and Oxigraph store. `dec` reads the current directory (walking up the tree if needed) to determine which stream it's acting on. No global mode switching, no `--stream` flag for every command, no ambiguity about which graph is being touched.

### 3.6 Cross-stream coordination

When an artifact crosses streams — for example, an oxi-events release becoming a dependency in decision-cli's stream — the artifact crosses the bus carrying its source stream identity. The consuming stream ingests it via its Discovery process. Each stream's graph remains internally consistent; cross-stream linkage is explicit and audited via PROV-O.

### 3.7 Identifying an instance at a glance

`dec status` surfaces the value stream identity prominently, including the definition source:

```
Value Stream:      decision-cli-development
Definition:        ./streams/decision-cli-development.ttl (sha256:a3f2…)
Terminal Value:    va:shipped-feature (bundled, ontology v0.1.0)
Authorized Goals:  ship, land
Graph Store:       ./.dec/store
Sessions (24h):    12
In-Flight:         2
```

Anyone landing in a working directory sees what the instance is geared toward, where its definition lives, and what state it's in.

---

## 4. Boundary with product-cli

The boundary is sharp and deliberate.

**product-cli does**: manages feature/ADR/TC/dep artifacts, builds the derived in-memory graph from front-matter, exports RDF, assembles curated context bundles (`product context`), runs preflight/gap/drift audits, computes fitness metrics, enforces graph health (`product graph check`), serves the engineering graph via stdio and HTTP MCP. The line in product-cli's PRD is explicit: *"Product does not invoke agents."*

**decision-cli does**: invokes agents, records sessions, routes artifacts between roles, manages model bindings, manages policy, runs the event substrate that makes graph mutations actionable, surfaces work to humans for checkpoints, and (eventually) improves itself based on measurement evidence. The line: *decision-cli does not own engineering artifact knowledge.* It calls product-cli for what product-cli already knows.

For slice 1, the integration is one-directional: decision-cli invokes product-cli via subprocess (`product context FT-XXX`, `product preflight FT-XXX`, etc.) and via product-cli's MCP write tools when an action produces an engineering artifact. product-cli remains oblivious to decision-cli's existence. Wiring product-cli to emit events through oxi-events is slice 2 work.

---

## 5. Architecture

### 5.1 The oxi-events crate

The event substrate is extracted as a separate, independently-versioned Rust crate: **oxi-events**. It is the only piece of decision-cli intended for community contribution.

The Stable Dependency Principle is the architectural rule: oxi-events depends only on substrates more stable than itself (oxigraph, tokio, tokio-stream, axum, serde, tracing). It has no dependency on decision-cli and no awareness of DDD-specific concepts (roles, bundles, sessions, policies). The framework's vocabulary is *mutations, subscriptions, events, delivery*. Everything else is application territory.

The framework crate lives inside decision-cli's workspace initially. Separate-repo extraction is deferred until the API has been pressure-tested by more than one consumer.

### 5.2 What oxi-events contains

- **`GraphWriter`** — the single mutation chokepoint over an Oxigraph store. All graph writes route through it. The writer owns the subscription registry and produces events on commit.
- **`Subscription`** — carries a SPARQL query, declared trigger types (which artifact-type mutations should re-evaluate it), a delivery handler, and an inline-vs-async classification.
- **Subscription evaluator** — on commit, evaluates affected subscriptions, diffs against prior result sets, emits delta events.
- **`Event`** — typed artifact with monotonic sequence number, outbox flag, PROV-O provenance back to the triggering mutation.
- **Outbox publisher** — background task that marks events `published` after successful delivery. Crash-safe: unpublished events are resumed on restart via SPARQL.
- **Delivery transports** — in-process tokio broadcast channels for co-located consumers, SSE via axum for remote consumers. Both serve the same logical stream.
- **Replay API** — SPARQL-based: "give me events for capability X since seq N." No separate replay infrastructure; the graph is the durable event log.

### 5.3 What decision-cli adds on top

- **Base ontology** embedded as a static asset: declares `dec:ValueStream`, `dec:ValueAction`, `dec:Goal`, `dec:Session`, `dec:Dispatch`, `dec:Event`, with SHACL shapes constraining required fields.
- **Bundled ValueAction definitions**: at minimum `va:shipped-feature` for slice 1, with `va:landed-pr`, `va:resolved-incident`, etc. added as later slices need them.
- **Bundled ValueStream templates**: `engineering-development`, plus others as needs emerge.
- **Init validation logic**: parse, SHACL-validate, resolve, cross-validate, persist, record provenance (§3.3).
- ValueStream and ValueAction artifacts seeded at init time, carrying PROV-O links to their definition sources.
- Role catalog, model catalog, policy artifacts (all first-class graph entities; full scope in later slices).
- Session records with PROV-O lineage, bundle hashes, model versions, all linked to the value stream via `dec:inStream`.
- Worker dispatch protocol (§7).
- v0 seed subscriptions bootstrapped on first startup.
- Subprocess integration with product-cli.
- A small CLI surface for human-triggered operations in slice 1.

### 5.4 The graph is the state

Graph-as-state over event-sourced. The current graph is the truth; events are derived signals that fire as side-effects of mutations. Named graphs preserve mutation history for audit; PROV-O links events back to their causing mutations and forward to the artifacts they triggered. There is no separate event log; there is one substrate, the graph.

Consequences:

- Replay = SPARQL over the historical graph.
- Consumer offsets = monotonic event sequence numbers, tracked by consumers.
- No event-sourced rebuild — current state is the truth; backups and named graph history cover recovery needs.

---

## 6. Slice 1 scope

### 6.1 In scope

- The oxi-events crate, as described in §5.2, with both delivery transports working.
- decision-cli binary (`dec`) with its own Oxigraph store for orchestration state.
- Embedded base ontology with SHACL shapes for `dec:ValueStream`, `dec:ValueAction`, and related entities.
- Bundled definitions: `va:shipped-feature` ValueAction, `engineering-development` ValueStream template.
- `dec init --template engineering-development` and `dec init --from <path>`: parse, SHACL-validate, resolve, cross-validate, persist, and record provenance per §3.3.
- ValueStream and ValueAction artifacts persisted in the orchestration store, with `dec:inStream` link enforced on every artifact write.
- **One role wired end-to-end: implementer.** Consumes a curated bundle for a feature, calls an LLM via a Python worker, produces a `CodeChange` artifact.
- Python code-writer worker conforming to the worker contract (§7).
- v0 seed subscriptions: "dispatch available for code-writer," "code-writer dispatch completed."
- Subprocess invocation of product-cli for bundle assembly and writes.
- Session record with PROV-O metadata, bundle hash, model version, timing, token counts, value stream link.
- Hardcoded model binding (one Claude model) — model catalog as a graph artifact is deferred.
- Hardcoded policy (one role, one capability, one model) — policy artifact is deferred.
- Explicit human triggering: `dec implement FT-XXX` starts the loop on demand.
- Minimal CLI surface: `dec init`, `dec status`, `dec implement`, `dec events tail`, `dec events since`, `dec session list`, `dec session show`, `dec health`.

### 6.2 Deliberately deferred to later slices

- Fetching definitions from URLs (slice 1 supports only bundled templates and local file paths).
- A definition registry / catalog server.
- ValueStream definitions that compose (extend a base template with overrides).
- Interpretation pairing (every action paired with a decision session that verifies its output).
- Feedback flow as a lifecycle class (gap / contradiction / unimplementable / scope-issue).
- Audits beyond SHACL conformance within the orchestration layer (preflight/gap/drift run via product-cli only).
- Model catalog as a first-class graph artifact.
- Policy as a first-class graph artifact with versioning and provenance.
- Multi-role flow (a second role consuming the first's output).
- The full goal-oriented CLI vocabulary (`dec drive`, `dec watch`, `dec schedule`).
- Cross-stream commands (`dec stream list`, `dec stream link`).
- Human checkpoints (Level 3 capability).
- The meta-loop (measurement-driven query/binding revision).
- product-cli emitting events through oxi-events (full reactive loop).
- Multi-stream operation in one binary invocation.
- Discovery system for OSS maintenance signals.
- Git integration (slice 1 writes files directly to the workspace).

### 6.3 Why this scope

Slice 1 proves the riskiest mechanical claims of the architecture: that Rust + Oxigraph carries the harness load, that the bundle-as-SPARQL-CONSTRUCT pattern is natural to author and maintain, that the stateless worker contract holds, that named-graph-per-session gives the audit story we want, that value stream scoping with schema-validated definitions enforces meaningful boundaries from the bootstrap moment, and that oxi-events can be cleanly extracted under SDP. Everything else is value added to a working foundation.

Slice 1 is also the last slice built entirely by humans. Slice 2 onward, the system processes its own feature_specs.

---

## 7. The worker contract

Workers are stateless functions: `bundle → artifact`. The contract is intentionally narrow:

- Workers receive a serialized bundle (markdown for slice 1) via the dispatch event payload.
- Workers do not talk to the graph. The harness assembles bundles and writes artifacts on the worker's behalf.
- Workers produce a structured output conforming to the role's output schema (Pydantic in Python, validated against SHACL by the harness on write).
- Workers report session telemetry: tokens, latency, tool call history, errors.
- Workers may emit feedback artifacts during execution — deferred to slice 2. Slice 1 workers report errors but do not emit structured feedback.

The Python code-writer worker for slice 1: receives a bundle for an implementer session, calls Claude with structured output, returns a `CodeChange` artifact describing the file paths it wrote and a diff summary. The worker writes files directly to the configured workspace directory. The harness records the session (linked to the value stream via `dec:inStream`), calls product-cli's MCP write tool to register the `CodeChange` in product-cli's graph, and marks the dispatch complete.

---

## 8. product-cli integration for slice 1

- **Reads**: subprocess invocation of `product feature next`, `product feature show`, `product context FT-XXX --depth N`, `product preflight FT-XXX`, `product graph stats`. Output is parsed from stdout (JSON where supported, structured text otherwise).
- **Writes**: invocation of product-cli's MCP write tools for `CodeChange` registration. Slice 1 may extend product-cli with a minimal new artifact type (`CodeChange`) if a feature_spec for that emerges from authoring.
- **No event subscription**: product-cli stays oblivious. When decision-cli needs to act on a product-cli state change, the human triggers it via `dec implement FT-XXX`.

The boundary is the operational realization of the architectural one: decision-cli treats product-cli as a service it consumes via the same interface humans use.

---

## 9. CLI surface for slice 1

The full `dec` vocabulary (`drive`, `watch`, `schedule`, `dispatch`) emerges over later slices. Slice 1 exposes a minimal subset focused on bootstrap, explicit human triggering of the implementer role, and inspection.

```
# Bootstrap — takes a definition reference, not raw strings
dec init --template <bundled-template-name>
dec init --from <path-to-definition.ttl>
                                       # parses, SHACL-validates, resolves ValueAction URI,
                                       # cross-validates authorized goals against the
                                       # ValueAction's compatible-goals, persists the
                                       # ValueStream + ValueAction artifacts, records
                                       # bootstrap session with PROV-O

# Identity and health
dec status                             # value stream identity, definition source + hash,
                                       # terminal value action, authorized goals, session
                                       # counts, in-flight count
dec health                             # liveness check

# Triggering
dec implement FT-XXX                   # trigger the implementer role on a feature
                                       # (shorthand for: dec dispatch role implementer FT-XXX)

# Inspection
dec events tail                        # subscribe to live events via SSE
dec events since <seq>                 # replay events from a sequence number
dec session list                       # recent sessions
dec session show <id>                  # session details with bundle hash and output ref
dec session log <id>                   # full PROV-O chain for a session
```

The shorthand `dec implement` is preserved as a convenience even in later slices — for any single-role direct dispatch, the verb form of the role is a valid shortcut for `dec dispatch role <role> <artifact>`.

---

## 10. What slice 2 will be (informally)

Not part of this bounds document, but worth flagging: slice 2 is the first time the system processes a feature_spec it didn't write itself. Likely slice 2 candidates:

- **Interpretation pairing** — every action session paired with a decision session that verifies the output. Closes the action→interpretation loop the DDD entity reference describes.
- **Feedback flow lifecycle** — first-class feedback artifacts with the controlled vocabulary (gap, contradiction, unimplementable, scope-issue) and routing.
- **product-cli event emission** — wire product-cli writes (via MCP) to emit oxi-events events. Converts the loop from explicit-trigger to reactive.

The slice 2 feature_spec is authored in product-cli using the structured authoring workflow. Drafting it is the next step after slice 1 starts running.

---

## 11. Author plan

This document is the bounds. The operational specification is authored next in two parts: the decision-cli orchestration store for value stream identity, and the product-cli graph for engineering artifacts.

### 11.1 Initialize the repo

```bash
# product-cli for engineering artifacts
mkdir -p docs && cd docs
# create product.toml configured for the decision-cli repo

# decision-cli for orchestration state
cd ..
dec init --from ./streams/decision-cli-development.ttl
```

The ValueStream definition document `./streams/decision-cli-development.ttl` is version-controlled in the repo so the stream's identity is itself a reviewable, audit-trackable artifact. `dec init` parses it, validates it against the embedded base ontology, resolves the referenced ValueAction URI against the bundled set, cross-validates authorized goals, then persists everything to `.dec/store/`.

After init, the orchestration store contains:

- The validated `ValueStream` artifact
- The resolved `ValueAction` artifact (a copy of the bundled definition)
- The v0 seed subscriptions
- The bootstrap session record (`dec:session/init-001`) with PROV-O links to the definition source, its content hash, the SHACL validation result, and the base ontology version

### 11.2 Author engineering artifacts in product-cli

Using `product author` mode with Claude as collaborator, seed feature_specs for the slice 1 units of work:

- oxi-events: GraphWriter
- oxi-events: Subscription registry and evaluator
- oxi-events: Event emission and outbox
- oxi-events: SSE delivery transport
- oxi-events: Replay API
- decision-cli: embedded base ontology + SHACL shapes
- decision-cli: bundled ValueAction and ValueStream template library
- decision-cli: init validation logic (parse / SHACL / resolve / cross-validate / persist / provenance)
- decision-cli: orchestration store and bootstrap (v0 subscriptions)
- decision-cli: value stream scope enforcement (the `dec:inStream` link, goal validation at command time)
- decision-cli: implementer role end-to-end
- decision-cli: slice 1 CLI commands (init, status, implement, events, session, health)
- Python code-writer worker

Seed ADRs for the structural decisions named in this document:

- oxi-events as a separate crate under SDP
- Graph-as-state over event-sourced
- Subscriptions as first-class graph artifacts
- PROV-O for events and sessions
- Value stream as a graph-resident scope, enforced at command time
- **Definition documents over raw strings for value stream init** — references canonical, schema-validated ValueAction and ValueStream definitions; raw `--stream`/`--value-action` strings are explicitly rejected as a design choice
- Embedded base ontology + bundled templates as the slice 1 distribution model
- Worker contract: stateless bundle-in / artifact-out
- product-cli integration via subprocess + MCP for slice 1
- Explicit human triggering in slice 1
- CLI shape: single-binary `dec` with namespaced subcommands
- Per-stream working directories (the git-style discovery model)

Seed test criteria for what "working" means at the slice boundary:

- exit-criteria: `dec init --template engineering-development` produces a `.dec/store/` containing the declared `ValueStream` and `ValueAction` artifacts, both reachable via SPARQL, both linked to the bootstrap session via PROV-O.
- exit-criteria: `dec init --from ./streams/decision-cli-development.ttl` produces an equivalent store, with the definition's content hash and file path recorded in the bootstrap session's PROV-O record.
- exit-criteria: `dec init --from <malformed.ttl>` fails before writing any state, with a clear SHACL violation message naming the missing or invalid fields.
- exit-criteria: `dec init --from <ttl-referencing-unknown-ValueAction-URI>` fails before writing state, naming the unresolvable URI.
- exit-criteria: `dec init --from <ttl-with-authorized-goal-not-in-compatible-set>` fails before writing state, naming the goal and the ValueAction's compatible set.
- exit-criteria: `dec status` displays the value stream identity, definition source path, content hash, and base ontology version matching what was used at init.
- exit-criteria: `dec drive <unauthorized-goal>` is refused with a message naming the stream's authorized goals (and noting the referenced ValueAction).
- exit-criteria: `dec implement FT-XXX` produces a `CodeChange` registered in product-cli's graph and a `Session` record in decision-cli's graph, both linked by PROV-O, with the Session linked to the ValueStream via `dec:inStream`.
- exit-criteria: events emitted by GraphWriter are queryable in the graph with monotonic sequence numbers.
- exit-criteria: outbox crash recovery — kill decision-cli mid-dispatch, restart, the in-flight dispatch resumes.
- exit-criteria: SSE delivery — a remote Python worker receives a dispatch event within one second of emission.
- invariant: every `Session` record links to its bundle hash, model version, and value stream via PROV-O.
- invariant: every `CodeChange` has a corresponding `Session` record reachable via PROV-O.
- invariant: every artifact in decision-cli's graph (Session, Goal, Dispatch, Event) carries a `dec:inStream` link.
- invariant: the bootstrap session record (`dec:session/init-001`) is present in every initialized store and is reachable via PROV-O from the ValueStream artifact.

### 11.3 The natural ordering

The two parts are not strictly sequential. `dec init` can run before any product-cli artifacts exist — it only seeds the orchestration store from the ValueStream definition. Engineering artifact authoring then proceeds in product-cli. The first time `dec implement FT-XXX` runs is the moment the two graphs first interact.

Once both parts are populated, the product-cli graph for decision-cli is the specification. This document fades into background context.

---

This is the bounds. The work begins by drafting the ValueStream definition document, running `dec init`, then authoring in product-cli.
