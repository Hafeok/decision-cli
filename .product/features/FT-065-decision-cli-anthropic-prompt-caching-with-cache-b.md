---
id: FT-065
title: 'decision-cli: Anthropic prompt caching with cache breakpoint placement and cache-hit fitness metric'
phase: 2
status: complete
depends-on:
- FT-054
- FT-057
- FT-061
- FT-062
adrs:
- ADR-001
- ADR-002
- ADR-004
- ADR-005
- ADR-008
- ADR-012
- ADR-013
- ADR-014
- ADR-015
- ADR-016
- ADR-017
- ADR-018
- ADR-020
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
- TC-114
domains:
- api
- networking
- observability
domains-acknowledged: {}
---

## Description

Implement Anthropic prompt-cache breakpoint placement on dispatches whose resolved capability has `endpoint = anthropic` and a non-null `cost_cache_hit_per_m` per PRD §9.4. The dispatcher places a single cache breakpoint between the bundle's stable prefix (focal artifact + linked ADRs/dependencies + tool definitions) and the per-attempt suffix (prior-attempt enrichment block from [ADR-034](ADR-034), per-step framing). The first dispatch in an escalation chain pays the 5-minute cache-write rate; subsequent dispatches within 5 minutes pay the 10×-cheaper cache-hit rate.

Track `cache_hit_rate` per session as a fitness function on the dispatcher: `input_tokens_cache_hit / total_input_tokens` (the three new fields from [FT-057](FT-057)). Target `> 70%` cache-hit rate on escalated Anthropic sessions per [ADR-037](ADR-037); a persistent rate below target signals the breakpoint is misplaced.

This is the cost-leverage half of [ADR-037](ADR-037): without caching, escalating to Opus on the third tier of a chain costs ~6-8× per token vs. Scaleway tier-2; with caching on the stable prefix, the marginal cost approaches the tier-2 per-token rate. Bundle prefixes are large and mostly static across escalation tiers — this is real money, not a micro-optimization.

## Functional Specification

### Inputs

- The Capability cache-rate fields from [FT-054](FT-054): `cost_cache_hit_per_m` and `cost_cache_write_5m` (both set on Anthropic capabilities `deep-reasoning`, `mid-reasoning`, `fast-reasoning`; both null on Scaleway).
- The dispatch payload from [FT-061](FT-061), already carrying the resolved `(endpoint, model, params)` triple.
- The escalation loop from [FT-062](FT-062) that produces the prior-attempt enrichment block.
- The session token-breakdown fields from [FT-057](FT-057): `input_tokens_base`, `input_tokens_cache_write`, `input_tokens_cache_hit`.
- The Anthropic API surface (via [FT-060](FT-060)'s `AnthropicRouter`): `messages.create` accepts `cache_control: {"type": "ephemeral"}` markers in `system` blocks and `messages[i].content` blocks.

### Outputs

- New module `core::dispatcher::caching`:
  ```rust
  pub struct CacheableBlock {
      pub content: String,
      pub cacheable: bool,           // true → cache_control marker set on Anthropic side
  }
  
  pub fn split_bundle_for_caching(
      bundle: &Bundle,
      prior_attempt: Option<&DispatchAttempt>,
  ) -> Vec<CacheableBlock>;
  // Returns: [stable_prefix (cacheable=true), per_attempt_suffix (cacheable=false)]
  // For first attempt in a chain, the suffix may be empty/minimal.
  ```
- The `AnthropicRouter` in [FT-060](FT-060) accepts the per-block cacheability hints (`Vec<CacheableBlock>`) and constructs the Anthropic request with `cache_control: {"type": "ephemeral"}` on the last block of the stable prefix. Scaleway and other non-cacheable endpoints ignore the markers (their callers do not see them — the dispatcher only emits them when the resolved capability has a non-null `cost_cache_hit_per_m`).
- Extended `core::dispatcher::dispatch_role` (the loop from [FT-062](FT-062)): before constructing the per-dispatch payload, the dispatcher checks `resolved_capability.cost_cache_hit_per_m.is_some()`. If yes, it calls `split_bundle_for_caching` and passes the result to the `AnthropicRouter` via [FT-060](FT-060)'s `CallParams` (a new optional field `cacheable_blocks: Option<Vec<CacheableBlock>>`). If no, the dispatcher passes the full bundle as a single non-cacheable string per the existing path.
- New fitness function on the metrics surface ([FT-024](FT-024)): `chain_cache_hit_rate(chain) -> f32` computed from session records via [FT-057](FT-057)'s helpers, surfaced through `dec metrics` and `dec session show <id>`.
- A warning emitted to telemetry when the running average `cache_hit_rate` across the last N Anthropic-escalated dispatches falls below 0.70 (configurable threshold, default per [ADR-037](ADR-037)).

### Stable prefix and per-attempt suffix

PRD §9.4 specifies what goes where:

**Stable prefix (cacheable):**
- System prompt (role definition, authority declaration from [FT-030](FT-030)).
- The focal artifact (the feature_spec / ADR / Capability being processed).
- Linked artifacts (ADRs, dependencies, TCs) included in the bundle.
- Tool definitions (the canonical OpenAI tool list from [FT-060](FT-060), translated to Anthropic format).

**Per-attempt suffix (not cached):**
- The prior-attempt enrichment block from [ADR-034](ADR-034) (different at each escalation step).
- The current step's specific framing ("Tier-N produced X; agree, refute, or refine").

The split lives in `split_bundle_for_caching`. The function is pure: bundle in, two blocks out. It does *not* read the graph or the prior session record beyond what's already in the `DispatchAttempt` passed to it.

### State

- No persistent state in the caching module itself. The breakpoint placement is deterministic given the bundle and the prior attempt.
- The Anthropic request payload grows by a `cache_control` marker on one content block; the response usage block returns the breakdown that [FT-057](FT-057) records on the session.

### Behaviour

1. The dispatcher resolves the capability per [FT-061](FT-061) and constructs the bundle markdown.
2. If `resolved.endpoint == Anthropic` and `resolved.cost_cache_hit_per_m.is_some()`:
   - Call `split_bundle_for_caching(bundle, prior_attempt)` → `[prefix, suffix]`.
   - Pass `CacheableBlocks::Some([prefix, suffix])` to the router via `CallParams`.
3. Else: pass the bundle as a single non-cacheable string (existing path).
4. The `AnthropicRouter` constructs the Anthropic `messages.create` request:
   - The stable prefix becomes a `system` block (or the start of `messages[0].content`) with `cache_control: {"type": "ephemeral"}` on its last segment.
   - The per-attempt suffix becomes the remaining `messages[0].content` (or a separate user message) without `cache_control`.
   - The first request in the chain pays the cache-write rate; subsequent requests within 5 minutes pay the cache-hit rate (Anthropic's API handles the cache state — the dispatcher does not).
5. The Anthropic response carries `usage.cache_creation_input_tokens` and `usage.cache_read_input_tokens`. The router extracts these and surfaces via `ModelResponse.tokens_cache_write` and `tokens_cache_hit` ([FT-060](FT-060)).
6. The harness writes the session record with the three token-breakdown fields per [FT-057](FT-057).
7. `dec session show <id>` reports the cache-hit rate for that session; `dec metrics --cache-hit-rate` reports the rolling average across Anthropic-escalated dispatches.
8. If the rolling rate drops below 0.70, the dispatcher logs a warning. The mitigation (rebalance the breakpoint placement) is a follow-up tuning, not an automatic recovery.

### Invariants

- The cache breakpoint is set if and only if the resolved capability has a non-null `cost_cache_hit_per_m`. No silent enablement on capabilities that lack the cost fields; no silent disablement on capabilities that have them.
- `split_bundle_for_caching` produces exactly two blocks; the prefix is cacheable, the suffix is not. (Future generalization to 4 breakpoints is out of scope per PRD §9.4.)
- The stable prefix content is byte-for-byte identical across attempts in the same escalation chain (this is what makes the cache work). The prefix is computed once per chain (in the first dispatch) and reused; subsequent dispatches in the chain recompute it and verify identity via SHA-256 — if it differs, the dispatcher logs a warning and proceeds (cache miss expected).
- Token counts in the session record sum correctly: `input_tokens_base + input_tokens_cache_write + input_tokens_cache_hit = total_input_tokens_billed`. Anthropic's usage block is the source of truth; the worker / router does not double-count.
- For Scaleway and any other capability without `cost_cache_hit_per_m`, the dispatch payload does not include cache markers and the session records `cache_write = cache_hit = 0`.

### Error handling

- The Anthropic API returning `usage` without `cache_creation_input_tokens` / `cache_read_input_tokens` (older API surface, or error response) → router writes `tokens_cache_write = 0` and `tokens_cache_hit = 0`, attributing all input to `tokens_in`. Cost reporting is conservative (over-estimates cost); cache-hit rate is 0.0 for that session. A warning is logged.
- A bundle that fails the prefix-identity check across an escalation chain (the prefix bytes changed between attempts — likely a bug in the dispatcher) → log warning, proceed without expecting cache hits; the cache_hit_rate metric will catch persistent issues.
- Bundle smaller than the minimum cacheable size (Anthropic has a minimum-tokens-per-cache-block threshold — currently ~1024 tokens) → the cache marker is set but Anthropic returns the block as non-cached; this is a silent miss, surfaced only by the metric. Not a failure.
- An Anthropic capability with `cost_cache_hit_per_m` set but no actual API caching support (impossible if the catalog matches reality, possible if Anthropic deprecates caching on a specific model) → operator updates the catalog; in the interim, the dispatcher still sends cache markers and Anthropic ignores them. Functional but wasteful.

### Boundaries

- **In scope.** `core::dispatcher::caching::split_bundle_for_caching`, the dispatcher-side cache-blocks plumbing, the `AnthropicRouter` cache_control placement, the cache-hit fitness function on the metrics surface, the warning when running rate drops below threshold.
- **Out of scope.** Scaleway caching — Scaleway does not currently support prompt caching; the dispatcher's check on `cost_cache_hit_per_m` automatically excludes Scaleway capabilities.
- **Out of scope.** Anthropic 1-hour cache TTL (the catalog only carries the 5-minute rate per PRD §5.2).
- **Out of scope.** Multi-breakpoint cache strategies (e.g. per-section caching of large ADRs). PRD §9.4: "If escalation chains grow longer or bundles grow more complex, additional breakpoints can be introduced as a follow-up optimization." Out of scope here; the single-breakpoint version captures the obvious win.
- **Out of scope.** Auto-rebalancing the breakpoint when cache-hit rate is low — the dispatcher logs and surfaces the metric; tuning is operator/meta-loop work (Phase 3+).

## Out of scope

- Per-feature cache-policy overrides (catalog field `cost_cache_hit_per_m` is the only switch; no per-bundle override).
- Caching of artifact responses (this PRD is about input caching only).
- 1-hour TTL cache writes (Anthropic offers a higher-cost 1h variant; not in the catalog and not used here).
- A `dec capability cache-stats` standalone command (subsumed by `dec metrics --cache-hit-rate` and `dec session show <id>`).
- Anthropic batch API integration (related cost-saver, but a separate feature_spec — batch is asynchronous; the dispatcher's escalation loop is synchronous).
