---
id: FT-134
title: 'decision-cli: dec serve — in-memory graph server for low-latency drives'
phase: 4
status: planned
depends-on:
- FT-110
- FT-111
- FT-119
adrs:
- ADR-047
- ADR-043
- ADR-036
- ADR-068
tests:
- TC-104
- TC-237
- TC-238
- TC-298
- TC-300
- TC-316
- TC-317
domains:
- api
- storage
domains-acknowledged:
  ADR-070: Server hosts orchestrator state but does not introduce new role tool surfaces — workers receive the same tool surfaces declared by the existing role catalog. The socket protocol is infrastructure beneath role-scoped dispatch, not a new tool.
  ADR-071: Per-workdir scope is inherited verbatim (one socket per .dec/serve.sock, 0600 perms, workdir-owner only); no new in-process worker tools — workers stay stateless subprocess per CLAUDE.md and ADR-008. Workspace containment + secrets blocking apply unchanged.
  ADR-072: Slice-1 of this feature lands the start/read/mutate/external-edit/shutdown/crash-recovery lifecycle; per the Behaviour section that's 6 distinct TCs (well above the 4-TC floor). Draft phase carries no TCs yet by convention; the 4+ floor will be satisfied before status complete.
---

## Description

A long-running `dec serve` daemon that holds the orchestration store + the
`.product/` graph in memory, exposes a Unix-socket API for reads and
SHACL-gated mutations, watches `.product/` for human edits, and writes
mutations through to the on-disk `.nq` dump and `.md` files.

Today every `dec` and `product` invocation re-loads the orchestration store
from `.dec/store/orchestration.nq`, re-parses every `.md` frontmatter it
needs, and shells out to `product preflight` / `product verify` for cross
ADR/coverage checks. For a single-feature `dec drive def-ready FT-XXX`
that's ~1–10s of per-invocation overhead; for the `--all` sweep over ~120
features it's effectively prohibitive (~20–30 minutes wall clock). The
substrate that needs to hold this state is already built — `oxi-events` is
explicitly described as the "graph-native event substrate" with axum +
tokio dependencies — so the missing piece is a process that hosts it.

`dec serve` is that process. When it's running, `dec drive`, `dec
implement`, `product preflight`, `product verify`, and the workers
themselves all talk to it over the socket instead of cold-loading from
disk. When it's not running, every existing CLI invocation falls back to
the current file-load path — no breakage, just slower (today's behaviour).

## Functional Specification

### Inputs

`dec serve` CLI verb:

| Flag | Default | Purpose |
|---|---|---|
| `--socket` | `.dec/serve.sock` | Unix-socket path to bind. |
| `--watch` | `true` | Tail `.product/` and `.dec/store/` for human edits. |
| `--flush-interval` | `60s` | Periodic `.nq` flush cadence (also flushed on graceful shutdown). |
| `--workdir` | CWD walk-up | Per ADR-012. |
| `--detach` | `false` | Background-mode (writes PID file alongside socket). |
| `--stop` | `false` | Best-effort: send SIGTERM to the PID in `<socket>.pid` and exit. |

Client-side discovery: each `dec` / `product` invocation checks for the
socket at `<workdir>/.dec/serve.sock`; if present and responsive
(< 100 ms ping), it routes reads/mutations through the server, otherwise
falls back to dump-load behaviour. No new env vars and no new explicit
client-side flags in slice-1 of this feature.

### Outputs

- A bound Unix socket speaking a small line-delimited request/response
  protocol (concrete framing left to ADR-NNN; this spec mandates only the
  semantic operations).
- Periodic flushes of the in-memory store to `.dec/store/orchestration.nq`
  on `--flush-interval`, on graceful shutdown, and on every N mutations
  (configurable threshold, default 100).
- A write-ahead log at `.dec/store/orchestration.wal` so crashes between
  flushes don't lose committed mutations.
- The existing `oxi-events` SSE stream surface available over the socket
  so subscribers (e.g. `dec events tail`) don't poll.

### State

The server holds:

| State | Source of truth | Sync direction |
|---|---|---|
| Orchestration `Store` | in-memory oxigraph + WAL | flushed → `.nq`; loaded ← `.nq` at start |
| `.product/` frontmatter index | in-memory; built from `.md` files | watched ← `.md` edits; written → `.md` on mutation |
| `last_flushed_seq` | in-memory + tail of `.nq` | persists across restarts |
| Active socket subscriptions | in-memory only | dropped on restart (per ADR-030 — server is a hot path, not durable subscribers) |

### Behaviour

The server is single-writer (one bound socket per workdir), multi-reader.
The core lifecycle:

1. **Start** — open the socket; refuse if the PID in `<socket>.pid` is
   still alive; load `.nq` dump; replay any unflushed WAL entries onto the
   restored store; index `.product/` frontmatter; start filesystem watchers
   on `.product/**/*.md`, `.product/requests.jsonl`, and
   `.dec/store/orchestration.nq`; begin accepting client connections.
2. **Read query** — serve entirely from memory. Existing inspector code
   paths (`feature_spec_completeness`, `preflight_status_for_feature`,
   etc.) get a server-mode adapter that proxies the request rather than
   re-reading frontmatter from disk.
3. **Mutation** — accepted through the existing `StreamWriter`
   chokepoint (ADR-005 invariant — server doesn't bypass SHACL or
   PROV-O); applied to memory; appended to the WAL; persisted to `.nq` on
   the configured cadence; if the mutation also implies a frontmatter
   change (status flip, new TC link), the corresponding `.md` file is
   rewritten on the same flush boundary.
4. **External file edit** — the watcher catches a human `vim` save on
   `FT-NNN.md`; the server re-parses the frontmatter, diffs it against
   its in-memory index, applies the implied delta through the same
   `StreamWriter` chokepoint, and surfaces an event on the subscription
   stream. If the human's edit conflicts with an in-flight mutation, the
   server logs the diff and refuses the file edit (operator-visible
   `dec serve status` warning) rather than silently overwriting.
5. **Graceful shutdown** — drain in-flight requests, flush WAL, write the
   final `.nq` dump, remove the socket + PID file, exit 0.
6. **Crash recovery** — on next start, the WAL replay from
   `last_flushed_seq` restores any post-flush, pre-crash committed
   mutations. A WAL whose checksum doesn't match aborts startup with a
   clear `dec serve doctor` invocation pointer.

### Invariants

- A read query served by the server is byte-identical to the same query
  served against a cold-loaded store from the same `.nq` dump + WAL.
- A mutation accepted by the server reaches both the in-memory store and
  the WAL before the response is sent (fsync on the WAL append).
- The server never writes to `.md` files outside the
  frontmatter; body edits remain a pure human activity.
- The socket is bound to `0600` permissions; only the workdir owner can
  read or write.
- Client fall-back is transparent: a CLI that hits a non-responsive
  socket should produce identical output to the same CLI run without
  the server.

### Error handling

- Socket connect timeout (> 100 ms) in a client → fall back to dump-load
  path; log a one-shot tracing warning so operators see they're paying
  the cold-load cost.
- Server WAL append fails → mutation response carries
  `error: durable_write_failed`; in-memory state is rolled back to the
  pre-mutation snapshot before the response is sent.
- Filesystem watcher drops events → next periodic `.product/` scan
  (every `--flush-interval`) reconciles the index against disk. Watchers
  are best-effort acceleration, not the source of correctness.
- Server crash mid-flush → next start detects an incomplete `.nq`
  (atomic-rename pattern: write to `.nq.tmp`, fsync, rename) and falls
  back to the prior good dump + WAL replay.

### Boundaries

- **Not a network server.** Unix socket only, scoped to one workdir, one
  user. No TLS, no auth, no remote. The `oxi-events` HTTP/SSE shape
  exists for future work but slice-1 ships the local socket only.
- **Not multi-writer.** Single `dec serve` per workdir; concurrent
  writes from multiple humans is out of scope (the GraphWriter lock that
  exists today already enforces single-writer, and this feature
  inherits that constraint).
- **Does not replace any worker.** Workers stay stateless (bundle in,
  artifact out, per CLAUDE.md and ADR-008). The server hosts the
  orchestrator's state; workers see a context bundle that is
  byte-identical to what they get today.
- **Does not own subscription durability.** Active subscriptions over
  the socket are dropped on restart; subscribers re-establish.
  Persistent subscription semantics are an `oxi-events` substrate
  concern, separately specified.
- **Does not split product-cli.** `product` is still the same binary
  with the same surface; it gains a thin server-mode adapter that
  routes graph reads through the socket when present.

## Out of scope

- The `dec serve --remote` HTTP/SSE surface for multi-host workflows.
  Local socket first; network surface authored as a follow-up feature
  once the local shape proves out.
- Auth/AuthZ for the socket beyond filesystem-permission scoping. Future
  multi-tenant work would author the role/token model separately.
- Live re-load of cross-cutting ADR scope changes. The server caches the
  computed preflight result per (feature, ADR-set fingerprint); a
  cross-cutting ADR scope change invalidates the cache. Full incremental
  re-resolution of every dependent computation across the graph is a
  separate optimisation feature.
- Distributed / multi-writer modes. Single-writer is a deliberate
  inherited constraint and not a target for this feature.
- A standalone `product serve` binary. This feature absorbs the
  product-cli read path through the same server (the two CLIs are
  already co-located per ADR-016); a separate product-cli daemon would
  reintroduce the dump-load latency we're trying to remove.

## Notes / motivation

This feature is motivated by FT-119 (DoR sweep): even after the
DoR-specific state-hash optimisation lands, the per-feature classify
pays ~1–8 s of orchestration-store load + `product preflight` shell
overhead, which compounds across a 120-feature sweep to 20–30 minutes.
A server that holds the store in memory and answers preflight + spec +
TC + VG queries directly drops that to ~1 ms per dimension lookup, well
below human latency. Without the server, every additional planner /
sweep we author inherits the same scaling problem.

This is also a structural fit for the value-stream architecture: the
substrate that needs to hold mutable graph state across long-running
sessions is the same substrate the meta-loop and the standing
observer-roles need (slice-2 and beyond). Authoring `dec serve` now is
authoring the surface those features will compose against.
