//! Session escalation chain + cost rollup helpers (FT-057).
//!
//! Three public helpers:
//!
//! - [`escalation_chain`] walks `dec:escalated_from` back to the root then
//!   forward via `dec:escalated_to` to the leaf, returning the chain in
//!   dispatch order. Detects orphaned references and surfaces a
//!   structured [`SessionError::ChainBroken`] without panicking.
//! - [`aggregate_chain_cost`] sums the cache-aware token-cost contribution
//!   of every session in a chain, grouped by currency (Scaleway tiers in
//!   EUR, Anthropic tiers in USD) per ADR-034's decision not to
//!   denormalise.
//! - [`cache_hit_rate`] computes the cache-hit fitness metric referenced
//!   in ADR-037; returns 0.0 (never NaN) when the session has no input
//!   tokens recorded.

use std::collections::BTreeMap;

use oxigraph::model::{NamedNode, Term};
use oxigraph::sparql::QueryResults;
use oxigraph::store::Store;
use thiserror::Error;

use crate::core::ontology::role_binding::TriggerSignal;

/// Read-side view of one session's FT-057 fields.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionView {
    /// Stable session IRI.
    pub iri: NamedNode,
    /// Prior session in the chain (if any).
    pub escalated_from: Option<NamedNode>,
    /// Next session in the chain (if any).
    pub escalated_to: Option<NamedNode>,
    /// Trigger that caused this escalation (if any).
    pub escalation_reason: Option<TriggerSignal>,
    /// Input tokens billed at `cost_input_per_m`.
    pub input_tokens_base: u64,
    /// Cache-write input tokens (Anthropic only).
    pub input_tokens_cache_write: u64,
    /// Cache-hit input tokens (Anthropic only).
    pub input_tokens_cache_hit: u64,
    /// Output tokens emitted by the session.
    pub output_tokens: u64,
    /// Capability IRI that resolved for this session (if recorded).
    pub capability: Option<NamedNode>,
}

/// Cost-rate context for one capability, used by [`aggregate_chain_cost`].
#[derive(Debug, Clone, PartialEq)]
pub struct CapabilityCostRates {
    /// Stable capability IRI.
    pub iri: NamedNode,
    /// `cost_input_per_m`, parsed.
    pub cost_input_per_m: f64,
    /// `cost_output_per_m`, parsed.
    pub cost_output_per_m: f64,
    /// `cost_cache_write_5m` if present (Anthropic only).
    pub cost_cache_write_5m: Option<f64>,
    /// `cost_cache_hit_per_m` if present (Anthropic only).
    pub cost_cache_hit_per_m: Option<f64>,
    /// Currency label (`EUR` or `USD`).
    pub currency: String,
}

/// Per-currency aggregate cost across a chain.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ChainCost {
    /// Base input tokens summed across the chain.
    pub base_tokens: u64,
    /// Cache-write input tokens summed across the chain.
    pub cache_write_tokens: u64,
    /// Cache-hit input tokens summed across the chain.
    pub cache_hit_tokens: u64,
    /// Output tokens summed across the chain.
    pub output_tokens: u64,
    /// Total cost in *each* currency that appears in the chain. Keys are
    /// `EUR` / `USD` literals; values are decimals.
    pub by_currency: BTreeMap<String, f64>,
}

/// Errors surfaced by the FT-057 read helpers.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum SessionError {
    /// `escalated_from` referenced an IRI not present in the store. The
    /// helper does NOT panic on a malformed chain.
    #[error("escalation chain broken at session <{session_id}>: missing referenced session <{missing_ref}>")]
    ChainBroken {
        /// IRI of the session whose `dec:escalated_from` points at a
        /// non-existent session.
        session_id: String,
        /// The missing referenced IRI.
        missing_ref: String,
    },
    /// SPARQL query failed.
    #[error("graph query failed: {0}")]
    QueryFailed(String),
    /// Session was not found in the store.
    #[error("session <{0}> not found in orchestration store")]
    NotFound(String),
}

/// Walk the escalation chain anchored at `session_id`. Returns the chain
/// in dispatch order (root first, leaf last). Calling with any node in
/// the chain returns the same list.
pub fn escalation_chain(
    store: &Store,
    session_id: &NamedNode,
) -> Result<Vec<SessionView>, SessionError> {
    // 1. Find the root by walking back via dec:escalated_from.
    let mut root = load_session(store, session_id)?;
    let mut visited: Vec<String> = vec![root.iri.as_str().to_string()];
    while let Some(prior) = &root.escalated_from.clone() {
        if visited.iter().any(|v| v == prior.as_str()) {
            // Cycle protection — should not happen given write-side
            // invariants, but better than infinite-loop.
            break;
        }
        let next =
            load_session_optional(store, prior)?.ok_or_else(|| SessionError::ChainBroken {
                session_id: root.iri.as_str().to_string(),
                missing_ref: prior.as_str().to_string(),
            })?;
        visited.push(next.iri.as_str().to_string());
        root = next;
    }

    // 2. Walk forward via dec:escalated_to.
    let mut chain: Vec<SessionView> = vec![root.clone()];
    let mut current = root;
    while let Some(next_iri) = current.escalated_to.clone() {
        if chain.iter().any(|s| s.iri == next_iri) {
            break; // cycle guard
        }
        let next =
            load_session_optional(store, &next_iri)?.ok_or_else(|| SessionError::ChainBroken {
                session_id: current.iri.as_str().to_string(),
                missing_ref: next_iri.as_str().to_string(),
            })?;
        chain.push(next.clone());
        current = next;
    }
    Ok(chain)
}

/// Compute the per-currency aggregate cost across a chain. `costs` must
/// resolve every distinct capability IRI cited by the chain; missing
/// capabilities are treated as zero-cost (cost reporting becomes
/// conservative in that case rather than failing).
#[must_use]
pub fn aggregate_chain_cost(
    chain: &[SessionView],
    costs: &BTreeMap<String, CapabilityCostRates>,
) -> ChainCost {
    let mut out = ChainCost::default();
    for s in chain {
        out.base_tokens = out.base_tokens.saturating_add(s.input_tokens_base);
        out.cache_write_tokens = out
            .cache_write_tokens
            .saturating_add(s.input_tokens_cache_write);
        out.cache_hit_tokens = out
            .cache_hit_tokens
            .saturating_add(s.input_tokens_cache_hit);
        out.output_tokens = out.output_tokens.saturating_add(s.output_tokens);
        let Some(cap) = &s.capability else { continue };
        let Some(rates) = costs.get(cap.as_str()) else {
            continue;
        };
        let base_cost = token_cost(s.input_tokens_base, rates.cost_input_per_m);
        let write_cost = token_cost(
            s.input_tokens_cache_write,
            rates.cost_cache_write_5m.unwrap_or(rates.cost_input_per_m),
        );
        let hit_cost = token_cost(
            s.input_tokens_cache_hit,
            rates.cost_cache_hit_per_m.unwrap_or(rates.cost_input_per_m),
        );
        let output_cost = token_cost(s.output_tokens, rates.cost_output_per_m);
        let total = base_cost + write_cost + hit_cost + output_cost;
        *out.by_currency.entry(rates.currency.clone()).or_insert(0.0) += total;
    }
    out
}

fn token_cost(tokens: u64, rate_per_m: f64) -> f64 {
    (tokens as f64) * rate_per_m / 1_000_000.0
}

/// Cache-hit rate per ADR-037 fitness metric:
/// `cache_hit / (base + cache_write + cache_hit)`. Returns `0.0` when
/// the denominator is zero (Scaleway / no-cache sessions).
#[must_use]
pub fn cache_hit_rate(s: &SessionView) -> f32 {
    let denom = s.input_tokens_base + s.input_tokens_cache_write + s.input_tokens_cache_hit;
    if denom == 0 {
        return 0.0;
    }
    (s.input_tokens_cache_hit as f32) / (denom as f32)
}

/// ADR-037 chain-wide cache-hit fitness metric (FT-065):
/// `Σ cache_hit / Σ (base + cache_write + cache_hit)` across every
/// session in `chain`. Returns `0.0` when the chain has no input
/// tokens recorded — never `NaN`.
///
/// The target band per ADR-037 is `> 0.70`; a chain whose computed
/// rate persistently falls below 0.70 indicates the cache breakpoint
/// is misplaced.
#[must_use]
pub fn chain_cache_hit_rate(chain: &[SessionView]) -> f32 {
    let (hits, denom) = chain.iter().fold((0u64, 0u64), |acc, s| {
        let denom = s
            .input_tokens_base
            .saturating_add(s.input_tokens_cache_write)
            .saturating_add(s.input_tokens_cache_hit);
        (
            acc.0.saturating_add(s.input_tokens_cache_hit),
            acc.1.saturating_add(denom),
        )
    });
    if denom == 0 {
        return 0.0;
    }
    (hits as f32) / (denom as f32)
}

/// FT-065 §Outputs warning threshold per ADR-037. The dispatcher emits
/// a warning when [`chain_cache_hit_rate`] across the last N
/// Anthropic-escalated dispatches falls below this value.
pub const CACHE_HIT_RATE_WARNING_THRESHOLD: f32 = 0.70;

/// True iff `rate` falls strictly below the ADR-037 warning threshold.
/// `0.0` (no data yet) does NOT trigger the warning — callers should
/// check that at least one chain has run before evaluating.
#[must_use]
pub fn cache_hit_rate_below_threshold(rate: f32) -> bool {
    rate > 0.0 && rate < CACHE_HIT_RATE_WARNING_THRESHOLD
}

fn load_session(store: &Store, iri: &NamedNode) -> Result<SessionView, SessionError> {
    load_session_optional(store, iri)?
        .ok_or_else(|| SessionError::NotFound(iri.as_str().to_string()))
}

fn load_session_optional(
    store: &Store,
    iri: &NamedNode,
) -> Result<Option<SessionView>, SessionError> {
    let q = session_view_query(iri);
    let results = store
        .query(q.as_str())
        .map_err(|e| SessionError::QueryFailed(e.to_string()))?;
    let QueryResults::Solutions(sols) = results else {
        return Err(SessionError::QueryFailed(
            "session lookup returned non-solution result".to_string(),
        ));
    };
    let mut view = empty_session_view(iri);
    let mut any_row = false;
    for sol in sols {
        let sol = sol.map_err(|e| SessionError::QueryFailed(e.to_string()))?;
        any_row = true;
        apply_solution(&mut view, &sol);
    }
    // Existence: if no triples whatsoever attach to the IRI, treat as absent.
    if !any_row || !session_exists(store, iri)? {
        return Ok(None);
    }
    Ok(Some(view))
}

fn session_view_query(iri: &NamedNode) -> String {
    format!(
        "PREFIX dec: <https://decision-cli.dev/ns#>
SELECT ?from ?to ?reason ?base ?write ?hit ?out ?cap WHERE {{
  {{
    OPTIONAL {{ <{iri}> dec:escalated_from ?from }}
    OPTIONAL {{ <{iri}> dec:escalated_to ?to }}
    OPTIONAL {{ <{iri}> dec:escalation_reason ?reason }}
    OPTIONAL {{ <{iri}> dec:input_tokens_base ?base }}
    OPTIONAL {{ <{iri}> dec:input_tokens_cache_write ?write }}
    OPTIONAL {{ <{iri}> dec:input_tokens_cache_hit ?hit }}
    OPTIONAL {{ <{iri}> dec:output_tokens ?out }}
    OPTIONAL {{ <{iri}> dec:capability ?cap }}
  }}
  UNION
  {{
    GRAPH ?g {{
      OPTIONAL {{ <{iri}> dec:escalated_from ?from }}
      OPTIONAL {{ <{iri}> dec:escalated_to ?to }}
      OPTIONAL {{ <{iri}> dec:escalation_reason ?reason }}
      OPTIONAL {{ <{iri}> dec:input_tokens_base ?base }}
      OPTIONAL {{ <{iri}> dec:input_tokens_cache_write ?write }}
      OPTIONAL {{ <{iri}> dec:input_tokens_cache_hit ?hit }}
      OPTIONAL {{ <{iri}> dec:output_tokens ?out }}
      OPTIONAL {{ <{iri}> dec:capability ?cap }}
    }}
  }}
}}",
        iri = iri.as_str(),
    )
}

fn empty_session_view(iri: &NamedNode) -> SessionView {
    SessionView {
        iri: iri.clone(),
        escalated_from: None,
        escalated_to: None,
        escalation_reason: None,
        input_tokens_base: 0,
        input_tokens_cache_write: 0,
        input_tokens_cache_hit: 0,
        output_tokens: 0,
        capability: None,
    }
}

fn apply_solution(view: &mut SessionView, sol: &oxigraph::sparql::QuerySolution) {
    if let Some(Term::NamedNode(n)) = sol.get("from") {
        view.escalated_from = Some(n.clone());
    }
    if let Some(Term::NamedNode(n)) = sol.get("to") {
        view.escalated_to = Some(n.clone());
    }
    if let Some(Term::Literal(lit)) = sol.get("reason") {
        view.escalation_reason = TriggerSignal::try_from_str(lit.value());
    }
    if let Some(Term::Literal(lit)) = sol.get("base") {
        if let Ok(n) = lit.value().parse() {
            view.input_tokens_base = view.input_tokens_base.max(n);
        }
    }
    if let Some(Term::Literal(lit)) = sol.get("write") {
        if let Ok(n) = lit.value().parse() {
            view.input_tokens_cache_write = view.input_tokens_cache_write.max(n);
        }
    }
    if let Some(Term::Literal(lit)) = sol.get("hit") {
        if let Ok(n) = lit.value().parse() {
            view.input_tokens_cache_hit = view.input_tokens_cache_hit.max(n);
        }
    }
    if let Some(Term::Literal(lit)) = sol.get("out") {
        if let Ok(n) = lit.value().parse() {
            view.output_tokens = view.output_tokens.max(n);
        }
    }
    if let Some(Term::NamedNode(n)) = sol.get("cap") {
        view.capability = Some(n.clone());
    }
}

fn session_exists(store: &Store, iri: &NamedNode) -> Result<bool, SessionError> {
    let q = format!(
        "ASK {{ {{ <{iri}> ?p ?o }} UNION {{ GRAPH ?g {{ <{iri}> ?p ?o }} }} }}",
        iri = iri.as_str(),
    );
    match store
        .query(q.as_str())
        .map_err(|e| SessionError::QueryFailed(e.to_string()))?
    {
        QueryResults::Boolean(true) => Ok(true),
        _ => Ok(false),
    }
}
