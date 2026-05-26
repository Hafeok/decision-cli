---
id: FT-066
title: 'code-writer: route Claude Code CLI through capability endpoint via y-router proxy for Scaleway'
phase: 2
status: complete
depends-on:
- FT-013
- FT-016
- FT-061
- FT-064
adrs:
- ADR-008
- ADR-017
- ADR-018
- ADR-021
- ADR-022
- ADR-023
- ADR-024
- ADR-025
- ADR-027
- ADR-033
- ADR-034
- ADR-035
- ADR-036
- ADR-037
tests:
- TC-115
- TC-116
- TC-117
- TC-118
domains:
- api
- networking
- security
domains-acknowledged: {}
---

## Description

[FT-064](FT-064) deferred the Scaleway implementer path. The verifier worker was migrated to capability-resolved dispatch (its `ModelCaller` already routes via [FT-060](FT-060)'s `ModelRouter`), but the code-writer worker still only spawns `claude -p` with the inherited shell environment — `endpoint` and `model_id` from the dispatch payload are read by the verifier but ignored by the implementer. The FT-064 spec discussed two options for closing that gap: build a parallel SDK-driven `ScalewayAgenticRunner`, or keep `claude -p` and proxy Scaleway through it. Option (a) was specced, never implemented.

This feature ships option (b). [Scaleway's Claude Code integration guide](https://www.scaleway.com/en/docs/generative-apis/reference-content/integrate-with-claude-code/) describes a now-standard pattern: run [y-router](https://github.com/luohy15/y-router) as a local Anthropic-↔-OpenAI translation proxy, point Claude Code at `http://localhost:8787` via `ANTHROPIC_BASE_URL`, and pass the Scaleway secret as `ANTHROPIC_AUTH_TOKEN`. Claude Code then talks to Scaleway-hosted models (`devstral-2-123b-instruct-2512`, `qwen3-coder-30b-a3b-instruct`, …) through the same agent harness it uses against the Anthropic API.

Adopting that pattern lets us keep a **single** implementer runner — `claude -p` — across both endpoints, and the dispatcher's resolved capability still decides which model is invoked. We avoid maintaining a second tool-calling loop in Python ([FT-060](FT-060)'s `ModelRouter` continues to serve the verifier / classifier / triager roles that are actually OpenAI-shaped Python workers; the implementer is not one of those).

The Scaleway capability seeds in `config/capabilities.yaml` (FT-058) already declare `endpoint: scaleway` and the dispatcher (FT-061) already pins `(endpoint, model_identifier)` onto the bundle JSON. What's missing is two things: (1) propagate `endpoint` into the Python worker's `DispatchPayload` schema, and (2) translate `(endpoint, model_id)` into the right Claude Code env vars at subprocess-spawn time. This feature closes both.

## Functional Specification

### Inputs

- The dispatch payload built by the Rust harness for the implementer role, today carrying `model_id: str` (FT-064) but **not** `endpoint`.
- The resolved `dec:Capability` for the implementer's active RoleBinding (FT-061 already loads this; the bundle builder must read its `endpoint` field too).
- `$SCW_SECRET_KEY` in the harness shell environment (required iff the active capability is Scaleway).
- `$DEC_YROUTER_URL` optional override; defaults to `http://localhost:8787`.
- A running y-router proxy reachable at `$DEC_YROUTER_URL` (the y-router process is out-of-band — operator runs `docker compose up -d` against the cloned repo).

### Outputs

**`DispatchPayload` schema extension** (`workers/code-writer/src/code_writer/models.py`):

- Add `endpoint: Literal["scaleway", "anthropic"]` field. Required, no default — the dispatcher must pin it (TC-113 invariant).

**Rust dispatcher bundle builder** (`crates/decision-cli/src/features/implement/bundle.rs` or the dispatch-payload assembly site):

- Populate `endpoint` from the resolved capability alongside `model_identifier`.

**Worker subprocess spawn** (`workers/code-writer/src/code_writer/_subprocess_runner.py`):

- Replace the bare `subprocess.Popen(args, ...)` with one that takes an explicit `env=` dict. Build the env from `os.environ.copy()` plus an overlay computed from `payload.endpoint` and `payload.model_id`:

  ```python
  def _claude_env_for(payload: DispatchPayload) -> dict[str, str]:
      env = os.environ.copy()
      if payload.endpoint == "anthropic":
          env["ANTHROPIC_MODEL"] = payload.model_id
          # Don't touch ANTHROPIC_BASE_URL — Claude Code's default Anthropic
          # path is what we want.
          return env
      if payload.endpoint == "scaleway":
          api_key = env.get("SCW_SECRET_KEY", "").strip()
          if not api_key:
              raise EndpointConfigError(
                  category="missing_credentials",
                  message="SCW_SECRET_KEY not set; cannot dispatch to Scaleway endpoint",
              )
          proxy_url = env.get("DEC_YROUTER_URL", "http://localhost:8787").rstrip("/")
          env["ANTHROPIC_BASE_URL"] = proxy_url
          env["ANTHROPIC_AUTH_TOKEN"] = api_key
          # Claude Code routes tier-by-tier; pin all four slots to the
          # capability's model so haiku/sonnet/opus aliases don't fall
          # back to default Anthropic ids the proxy can't translate.
          for var in (
              "ANTHROPIC_MODEL",
              "ANTHROPIC_DEFAULT_HAIKU_MODEL",
              "ANTHROPIC_SMALL_FAST_MODEL",
              "ANTHROPIC_DEFAULT_SONET_MODEL",
              "ANTHROPIC_DEFAULT_OPUS_MODEL",
          ):
              env[var] = payload.model_id
          return env
      raise EndpointConfigError(
          category="unsupported_endpoint",
          message=f"unknown endpoint {payload.endpoint!r}",
      )
  ```

- `EndpointConfigError` is a new structured error that surfaces as `WorkerError(category="endpoint_config", retryable=False, …)` so the harness can render it in the session log and the operator can fix the env before re-dispatching.

- Reachability probe: before spawning, if `endpoint == "scaleway"`, attempt a quick `GET <proxy_url>/` with a ≤ 2 s timeout. If it fails, return a structured `WorkerError(category="proxy_unreachable", message="y-router not responding at <url>", retryable=True)`. The probe is best-effort — it must not delay successful dispatches by more than ~50 ms in the warm path.

**Preflight extension** (`core/bootstrap` or the FT-016 worker preflight audit):

- After role-binding bootstrap, for every active binding whose default capability is Scaleway: emit a warning if `$SCW_SECRET_KEY` is unset, and a warning if `$DEC_YROUTER_URL` (default `http://localhost:8787/`) doesn't respond to a HEAD request within 1 s. Warnings, not fatal errors — operators may legitimately bring the proxy up *after* `dec init`.

**Operator helper** (`scripts/start-y-router.sh`):

- One-command bring-up: clones https://github.com/luohy15/y-router on first run (into `.dec/y-router/` so the working tree stays clean), writes `wrangler.toml` with `OPENROUTER_BASE_URL = "https://api.scaleway.ai/v1"`, runs `docker compose up -d`, polls `/` until it responds (≤ 30 s budget), then exits 0. Idempotent — re-running on an already-running proxy is a no-op.

- The script is operator-facing convenience, **not** a worker dependency. The worker only requires that *something* is listening at `$DEC_YROUTER_URL`; how it got there is not the worker's business.

### Behaviour

1. Operator runs `scripts/start-y-router.sh` once per machine (or stands the proxy up themselves).
2. Operator sets `SCW_SECRET_KEY` in their shell.
3. `dec implement FT-XXX` resolves the implementer's active RoleBinding → `code-writer` capability (FT-058 seed, `endpoint: scaleway`, `model_identifier: qwen3-coder-30b-a3b-instruct` or whichever is active at the time).
4. The dispatcher (FT-061) assembles the dispatch payload, **including the resolved `endpoint`** alongside `model_id`.
5. The harness spawns `python -m code_writer run-once` with the payload on stdin.
6. The worker computes `_claude_env_for(payload)`, performs the proxy reachability probe (Scaleway path only), and spawns `claude -p` with the computed env.
7. Claude Code talks to `localhost:8787` (y-router) using its existing Anthropic API client, authenticated with the Scaleway key. y-router translates each request to OpenAI-shaped JSON and forwards to `https://api.scaleway.ai/v1`.
8. Tool calls, edits, and the final result stream back through y-router → Claude Code → the worker's `_subprocess_runner` exactly as today. The downstream `CodeChange` assembly is unchanged.
9. If the implementer escalates to `endpoint: anthropic` (e.g. `deep-reasoning` for `stakes_foundational`), `_claude_env_for` takes the Anthropic branch: no `ANTHROPIC_BASE_URL` override, `ANTHROPIC_MODEL` pinned to `claude-opus-4-7` (or whichever the capability says). Claude Code uses its native Anthropic API path.

### Invariants

- The model id passed to `claude -p` (via env) always originates from a resolved capability; never a hardcoded string in the worker. Extends TC-113.
- When `endpoint == "scaleway"`, all five `ANTHROPIC_*_MODEL` env vars carry the same Scaleway model id. Mismatched slots would let Claude Code fall through to a default Anthropic id (e.g. `claude-haiku-4-5`) that the proxy can't translate, surfacing as a 404 from Scaleway.
- When `endpoint == "scaleway"`, absence of `$SCW_SECRET_KEY` produces a structured `EndpointConfigError` *before* the subprocess spawns. Silent fallback to the Anthropic path would burn through Anthropic credits on work meant for Scaleway.
- The Anthropic path **never** sets `ANTHROPIC_BASE_URL`. Setting it (even to the official `api.anthropic.com`) would route through the proxy unnecessarily.
- The proxy reachability probe never blocks more than 2 s; warm-path dispatches feel the same as today.
- The y-router process is treated as out-of-band infrastructure. The worker assumes it exists at `$DEC_YROUTER_URL`; the worker is **not** responsible for starting, monitoring, or restarting it.

### Error handling

| Condition | Response category | Retryable | Action |
|---|---|---|---|
| `payload.endpoint == "scaleway"` and `SCW_SECRET_KEY` missing | `endpoint_config` | no | operator fixes shell env |
| `payload.endpoint == "scaleway"` and proxy probe fails | `proxy_unreachable` | yes | operator starts proxy; harness can re-dispatch |
| `payload.endpoint` is neither `scaleway` nor `anthropic` | `endpoint_config` | no | catalog/schema bug — surfaces a malformed capability |
| `claude -p` exits non-zero (e.g. 404 from upstream Scaleway) | existing `subprocess_failed` | no | unchanged from current path |

## Test Criteria

(Authored separately via `product test new` after this spec lands. Sketch:)

- **Scenario TC** — Scaleway env injection. Given `payload.endpoint = "scaleway"`, `payload.model_id = "qwen3-coder-30b-a3b-instruct"`, and `SCW_SECRET_KEY = "scw-test-key"` in the worker env: the dict returned by `_claude_env_for(payload)` contains `ANTHROPIC_BASE_URL=http://localhost:8787`, `ANTHROPIC_AUTH_TOKEN=scw-test-key`, and all five `ANTHROPIC_*_MODEL` vars set to `qwen3-coder-30b-a3b-instruct`. Runner: `pytest workers/code-writer/tests/test_claude_env_routing.py::test_scaleway_env`.

- **Scenario TC** — Anthropic env passthrough. Given `payload.endpoint = "anthropic"`, `payload.model_id = "claude-opus-4-7"`: `_claude_env_for(payload)` sets `ANTHROPIC_MODEL=claude-opus-4-7` and does **not** set `ANTHROPIC_BASE_URL` or `ANTHROPIC_AUTH_TOKEN` (Claude Code's existing Anthropic path stays intact). Runner: same pytest file, `test_anthropic_env`.

- **Scenario TC** — missing credential surfaces structured error. Given `payload.endpoint = "scaleway"` and `SCW_SECRET_KEY` unset: `_claude_env_for(payload)` raises `EndpointConfigError(category="missing_credentials")`; the calling code converts it to `WorkerResponse.status="error"` with `error.category="endpoint_config"`, `retryable=False`. Runner: same file, `test_missing_scw_key`.

- **Invariant TC** (extends TC-113) — no hardcoded model strings in `_subprocess_runner.py` or `_claude_env_for`. Every model literal must originate from `payload.model_id`. Runner: `bash tests/scripts/tc-XXX-claude-env-no-hardcoded-models.sh`.

- **Scenario TC** — end-to-end through stub proxy. Spin up a 30-line Python stub at `http://localhost:8788` that records each incoming request and returns a canned `claude -p`-compatible response. With `DEC_YROUTER_URL=http://localhost:8788`, dispatch a feature; assert the stub received exactly one POST with `Authorization: Bearer scw-test-key`. Runner: `pytest workers/code-writer/tests/test_yrouter_integration.py::test_request_carries_scw_auth`.

## Out of scope

- **Running y-router in CI.** The proxy is an operator dependency, not a deliverable of this feature. CI uses `CODE_WRITER_STUB=1` (existing FT-013 path) and asserts env-building correctness without round-tripping live API calls.
- **Killing the FT-060 `ModelRouter`.** The router still serves the verifier, classifier, architect, and feedback-triager roles — all Python workers calling Scaleway directly. This feature only changes the implementer (the one role that uses Claude Code).
- **Multi-tenant proxy.** y-router runs as `localhost:8787` per developer / per CI runner. A shared proxy is feasible (just set `DEC_YROUTER_URL` to its address) but not delivered here.
- **OAuth or rotating credentials for Scaleway.** Static `SCW_SECRET_KEY` only.
- **Anthropic model overrides via env.** The Anthropic path currently inherits whatever Claude Code defaults to; this feature pins it explicitly to `payload.model_id` (so the Anthropic claude-opus-4-7 path uses the catalog's pinned id), but doesn't add operator-level env overrides — those would defeat capability-driven routing.

## Implementation order

1. Extend `DispatchPayload` with `endpoint: Literal["scaleway", "anthropic"]`. Migrate the verifier worker's payload-parsing (already reads `endpoint` for `ModelRouter`) so the field shape matches across workers.
2. Update the Rust dispatch-payload assembly to populate `endpoint` from the resolved capability (probably a one-line addition where `model_id` is already written).
3. Add `_claude_env_for(payload)` and `EndpointConfigError` in `_subprocess_runner.py`. Wire it into the subprocess spawn via `env=`.
4. Add the proxy reachability probe with a 2 s budget. Keep it best-effort: probe failure → structured `proxy_unreachable` error; probe disabled when `endpoint == "anthropic"`.
5. Extend the FT-016 worker preflight: add Scaleway credential + proxy reachability checks (warning, not fatal).
6. Author `scripts/start-y-router.sh` and document the one-time setup in CLAUDE.md.
7. Land the five TCs sketched above.

## References

- [Scaleway: Integrate Claude Code with Scaleway Generative APIs](https://www.scaleway.com/en/docs/generative-apis/reference-content/integrate-with-claude-code/)
- [y-router GitHub](https://github.com/luohy15/y-router)
- [FT-064](FT-064) — migration cleanup; specced the dual-runner approach this feature supersedes
- [FT-061](FT-061) — capability resolution; populates the bundle this feature consumes
- [FT-013](FT-013) — the Python code-writer worker this feature extends
- [ADR-037](ADR-037) — Scaleway as default endpoint for cost-dominant roles
- [ADR-033](ADR-033) — capability-based routing
- [ADR-013](ADR-013) — code structure (file/function length limits apply to new helpers)
- [ADR-008](ADR-008) — workers are stateless (env injection is request-scoped, no worker state mutation)
