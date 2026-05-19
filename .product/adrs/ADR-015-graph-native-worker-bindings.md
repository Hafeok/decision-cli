---
id: ADR-015
title: Graph-native worker bindings
status: accepted
features: []
supersedes: []
superseded-by: []
domains: []
scope: domain
content-hash: sha256:6c7db92741d1b0b9a7058a691437c163785a664ed5d53dd74ad7bd19fa9ce3c6
---

## Context

In slice 1 a worker is anything `dec` can spawn that conforms to ADR-008's stateless `bundle → artifact` contract. The harness finds workers through an environment-driven search order implemented inline in `crates/decision-cli/src/implement.rs:701-718`:

```
--worker-command  >  $CODE_WRITER_CMD  >  which("code-writer")  >  python3 -m code_writer.main
```

This works for the one-worker, one-role world of slice 1, but it has three structural problems that will compound as the system grows:

1. **The environment is the source of truth.** What `dec implement` actually invoked is a function of `$PATH` ordering, env vars, and `which`-lookup timing at dispatch time. There is no audit trail of *which* `code-writer` binary processed a given dispatch — only that "a worker ran". For a system whose central claim is graph-native auditability (ADR-002, ADR-004), this is a hole.
2. **Roles and workers are conflated.** The resolution chain is hard-coded to look for the string `code-writer`. When `reviewer`, `architect`, `verifier`, and meta-loop workers arrive in later slices, each will want its own resolution chain, its own env var, its own fallback command. Inlining N copies of the slice 1 chain is a non-starter.
3. **No version, no hash, no compatibility check.** Two operators running the same `dec init` against the same definition can end up with two different code-writer binaries (different `uv tool install` timing, different Python interpreters, different worker source revisions). The graph records the bundle hash and the session, but not the executable identity. Reproducing a failed implementer run on another machine is not currently possible from the graph alone.

FT-016 introduces a slice 1 mitigation: a build-time **worker manifest** embedded in the `dec` binary, plus a `dec init` / `dec doctor` audit surface. That feature handles "is a worker present?" — but it does so by *recomputing* the answer from the environment each time. It deliberately does not write to the graph; it does not yet support multiple worker versions side by side; it does not yet support operator-declared worker bindings beyond the env-var escape hatch. The audit is correct as far as it goes, but it is not yet an authoritative record.

The framing question for this ADR is: **when the operator runs `dec implement`, where does the answer to "which executable should run this role?" come from?** Slice 1 says "the environment, audited at init time." This ADR proposes: **the graph, recorded at worker-registration time, referenced by every dispatch.**

See `docs/ddd/Implementing_DDD.md` (model bindings as a graph-resident concept) and ADR-002 (graph-as-state over event-sourced) for the principle this builds on, and FT-016 for the slice 1 bridge surface this supersedes.

## Decision

**Workers become first-class graph entities.** A `dec:WorkerBinding` artifact records, per role, the identity and invocation of the worker that the orchestration store considers authoritative. `dec implement` resolves the worker by querying the graph, not the environment.

The binding shape (subject to refinement during slice 2 design):

```
dec:WorkerBinding
  dec:role            "code-writer"             # the role this binding satisfies
  dec:kind            "uv-tool" | "pipx" | "cargo-install" | "oci-image" | "raw-command"
  dec:command         "code-writer run-once"    # invocation string (kind-dependent semantics)
  dec:version         "0.3.1"                   # operator-declared or installer-derived
  dec:contentHash     "sha256:..."              # of the deployed artifact, for reproducibility
  dec:installSource   "./workers/code-writer"   # path | wheel URL | OCI ref | git ref
  dec:installedAt     xsd:dateTime              # provenance
  dec:installedBy     "operator:hafeok"         # who registered it
  dec:supersedes      dec:WorkerBinding/...     # previous binding for this role (chain)
```

A small command surface manages the lifecycle:

- `dec worker install <role>` — runs the installer adapter for the role's manifest entry (uv-tool for `code-writer`, others as workers are added), then registers the resulting binding in the graph in a single transaction.
- `dec worker register <role> --command "<cmd>" --version "<v>"` — registers an externally-installed worker (the escape hatch for operators with bespoke setups; the operator vouches for the binding).
- `dec worker list` — shows the active binding per role.
- `dec worker remove <role>` — supersedes the active binding with a tombstone, leaving the chain intact for audit.

At dispatch time, `dec implement` issues a SPARQL lookup for the active `dec:WorkerBinding` whose `dec:role` matches the dispatched role, and invokes `dec:command` exactly as recorded. The environment-driven search order from FT-016 collapses into a single source of truth.

Compatibility checks become straightforward and graph-visible:

- The dispatched bundle records the binding it resolved against.
- A `dec doctor` (originally introduced in FT-016) reports the registered binding alongside its current resolvability — if the recorded command no longer points at an installed binary, the doctor surfaces the drift.
- A future "minimum worker version" constraint on a value action or feature_spec is a SHACL shape against the binding, not a runtime check sprawled across `implement.rs`.

The embedded worker *manifest* from FT-016 is retained and reframed: it becomes the **installer catalogue** that `dec worker install` consults, not the *resolver* `dec implement` consults. Resolution is graph-only after this ADR lands.

## Consequences

**Positive:**

- The graph answers "which executable ran this session?" alongside "which bundle was used?" and "which model was invoked?" — closing the reproducibility loop. Worker identity joins the PROV-O chain (ADR-004).
- Multiple roles scale uniformly. Adding a `reviewer` worker is "declare it in the installer catalogue, run `dec worker install reviewer`", not "extend the inline resolution chain in `implement.rs`".
- Operator setup becomes a single command per worker, idempotent and recorded. `dec init` no longer needs to print install hints as a separate UX surface; it points at `dec worker install --all` (or similar) and the graph captures the rest.
- Version pinning, version skew detection, and supersession audit fall out of the graph schema rather than requiring new code paths.
- Air-gapped and reproducibility-sensitive deployments gain a hand-hold: a value-stream export that includes worker bindings can be re-imported on another host, and `dec worker install` can verify the recorded content hash against the deployed artifact.

**Negative / accepted costs:**

- New surface: a binding schema, SHACL shapes, four CLI commands, and at least one installer adapter (`uv-tool`) for slice 2. More if `pipx`/`cargo-install`/OCI are wanted at the same time — and they will be, because the second worker that arrives is unlikely to be Python.
- The installer-adapter layer is the riskiest part. It must run third-party installers safely (no shell injection through `dec:installSource`), record actual installer output, and tolerate partial failures (binary installed, graph write failed) without orphaning state.
- Operators who today edit `$CODE_WRITER_CMD` per shell session need a migration story. `dec worker register --command "$CODE_WRITER_CMD"` is the obvious answer, but the env-var escape hatch should not be quietly removed in the same slice that introduces bindings — FT-016's resolution chain remains as a *fallback* until bindings are stable.
- The graph becomes load-bearing for `dec implement` startup. A corrupted or unmigrated store can prevent dispatch — a class of failure that today's environment-driven resolution does not have. SHACL validation and a clear `dec doctor` error path mitigate this, but it is a real shift in failure surface.

**Boundary enforcement:**

- The binding schema lives in the embedded base ontology (ADR-007), not in the worker manifest. The manifest tells `dec worker install` how to install; the ontology tells the graph how to validate the resulting binding.
- Installer adapters live in `decision-cli` (the orchestration crate), not in `oxi-events`. The SDP boundary from ADR-001 is unchanged — `oxi-events` does not learn about workers.
- Workers themselves remain stateless (ADR-008). Bindings record how the harness invokes workers; they do not change what workers do.

## Relationship to FT-016 and onward path

FT-016 is the slice 1 bridge: it formalises the resolution chain, adds the audit UX, and embeds the worker manifest. This ADR proposes the slice 2+ destination: the resolution chain collapses into a graph lookup, the manifest becomes an installer catalogue, the audit becomes a property of the graph rather than a recomputation.

Concretely, when this ADR is accepted:

- FT-016's resolution chain moves *behind* `dec worker install` (as the installer's `which`/probe step) and *behind* a final fallback (until bindings reach parity).
- `dec doctor` gains a new section: "registered bindings vs. resolved invocations", with drift surfaced explicitly.
- `dec implement` is refactored a second time: from "call `worker::resolve(role)` on the environment" to "call `worker::lookup(role)` on the graph, then verify the recorded command resolves".

## Status

Proposed. Implementation will be specified by a follow-on feature_spec in slice 2, after FT-016 lands and the operator UX is validated against real second-worker scenarios. The slice 1 surface (FT-016) is intentionally compatible with this direction — neither superseded nor blocked by this proposal.
