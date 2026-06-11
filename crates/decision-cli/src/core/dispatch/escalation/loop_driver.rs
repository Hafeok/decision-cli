//! Dispatcher loop entry point per FT-062 §Outputs.

use oxi_events::Mutation;
use oxigraph::model::NamedNode;
use oxigraph::store::Store;

use crate::core::bundle::Bundle;
use crate::core::dispatch::caching::{should_cache, split_bundle_for_caching, CacheableBlock};
use crate::core::dispatch::capability_resolver::{resolve_default_capability, ResolvedCapability};
use crate::core::ontology::capability::{query_by_iri, CapabilityStatus};
use crate::core::ontology::role_binding::active_for_role;
use crate::StreamWriter;

use super::bundle_enrich::enrich_bundle_with_prior_attempt;
use super::quads::{build_enriched_bundle_quads, build_session_linkage_quads};
use super::signals::collect_signals;
use super::triggers::{find_next_escalation_step, pick_reason};
use super::types::{AttemptTokens, DispatchAttempt, EscalationError, SessionId, WorkerResult};

/// Outcome of [`dispatch_role`] — the chain of attempts plus the
/// chain head id and a flag indicating whether the binding's
/// escalation list was exhausted.
#[derive(Debug)]
pub struct ChainResult {
    /// Every attempt in chain order (root first, leaf last).
    pub attempts: Vec<DispatchAttempt>,
    /// IRI of the first attempt's session — the chain head.
    pub chain_head: SessionId,
    /// True iff the binding's escalation list was exhausted with the
    /// final attempt still triggering (FT-062 §Termination).
    pub escalation_exhausted: bool,
    /// Final attempt's worker result.
    pub final_result: WorkerResult,
}

impl ChainResult {
    /// IRI of the last (leaf) session in the chain.
    #[must_use]
    pub fn leaf(&self) -> Option<&SessionId> {
        self.attempts.last().map(|a| &a.session_id)
    }
}

/// Worker invocation abstraction. Per ADR-008, workers consume a
/// bundle and produce a structured result. FT-062 escalation lives
/// outside the worker; this trait is the seam through which the
/// dispatcher receives the result, leaving worker plumbing (subprocess,
/// model router) to FT-060 / FT-064.
pub trait WorkerRunner {
    /// Invoke the worker once with `bundle` + resolved capability,
    /// returning a structured attempt.
    ///
    /// `session_id` is minted by the dispatcher up-front so the
    /// session record's IRI is decided before the worker runs.
    fn run(
        &mut self,
        role_id: &str,
        bundle: &Bundle,
        capability: &ResolvedCapability,
        prior_attempts: &[DispatchAttempt],
        session_id: &SessionId,
    ) -> Result<DispatchAttempt, EscalationError>;

    /// Optional hook invoked by the dispatcher with the cache breakpoint
    /// split per FT-065. The dispatcher calls this **before** [`Self::run`]
    /// when [`should_cache`] returns true for the resolved capability;
    /// otherwise the hook is not invoked. Workers that do not care about
    /// caching (the default impl) ignore the blocks; the production
    /// `AnthropicRouter` (FT-060) forwards them as `cache_control`
    /// markers on the request.
    fn on_cache_blocks(&mut self, _blocks: Vec<CacheableBlock>) {}
}

/// Mint a deterministic session IRI from `(role, bundle_hash, tier)`.
#[must_use]
pub fn mint_session_iri(role: &str, bundle_hash: &str, tier: u32) -> SessionId {
    NamedNode::new_unchecked(format!(
        "https://decision-cli.dev/ns/session/{role}/{hash}/tier-{tier}",
        role = role,
        hash = bundle_hash,
        tier = tier,
    ))
}

/// Execute the full dispatch chain for `role_id`.
///
/// 1. Resolve default capability (FT-061).
/// 2. Run worker via `runner`.
/// 3. Persist session + (if escalated) bidirectional chain edges.
/// 4. Collect signals.
/// 5. If a matching escalation step exists, enrich bundle and loop;
///    otherwise return.
pub fn dispatch_role<R, F>(
    store: &Store,
    writer: &StreamWriter,
    role_id: &str,
    initial_bundle: Bundle,
    runner: &mut R,
    mut tokens_fn: F,
) -> Result<ChainResult, EscalationError>
where
    R: WorkerRunner,
    F: FnMut(&DispatchAttempt) -> AttemptTokens,
{
    let binding =
        active_for_role(store, role_id)?.ok_or_else(|| EscalationError::NoActiveBinding {
            role_id: role_id.to_string(),
        })?;
    let mut state = LoopState::initial(store, role_id, initial_bundle)?;

    loop {
        let session_id = mint_session_iri(role_id, &state.bundle.hash, state.tier);
        // FT-065: emit cache_control markers only when the resolved
        // capability has a non-null cost_cache_hit_per_m and is Anthropic.
        // The split is deterministic; the prefix is byte-stable across
        // attempts in the same chain (FT-065 §Invariants).
        if should_cache(&state.capability) {
            let prior_attempt = state.attempts.last();
            let blocks = split_bundle_for_caching(&state.bundle, prior_attempt);
            runner.on_cache_blocks(blocks);
        }
        let attempt = runner.run(
            role_id,
            &state.bundle,
            &state.capability,
            &state.attempts,
            &session_id,
        )?;
        let tokens = tokens_fn(&attempt);
        commit_session(
            writer,
            &attempt,
            tokens,
            state.prior_session.as_ref(),
            state.prior_reason,
        )?;
        state.record_chain_head(&attempt);

        let signals = collect_signals(&state.bundle, &attempt, state.tier);
        state.attempts.push(attempt.clone());

        let Some(next_step) = find_next_escalation_step(&binding, &state.capability, &signals)
        else {
            return Ok(state.into_chain_result(attempt.result));
        };

        let reason = pick_reason(&next_step, &signals);
        let new_cap = resolve_capability_for_step(store, &next_step.step_capability)?;
        commit_enriched_bundle(writer, &state.bundle, &attempt, state.tier)?;
        state.advance(attempt, reason, new_cap);
    }
}

struct LoopState {
    bundle: Bundle,
    bundle_for_supersession: Bundle,
    capability: ResolvedCapability,
    tier: u32,
    attempts: Vec<DispatchAttempt>,
    chain_head: Option<SessionId>,
    prior_session: Option<SessionId>,
    prior_reason: Option<crate::core::ontology::role_binding::TriggerSignal>,
}

impl LoopState {
    fn initial(
        store: &Store,
        role_id: &str,
        initial_bundle: Bundle,
    ) -> Result<Self, EscalationError> {
        let capability = resolve_default_capability(store, role_id)?;
        Ok(Self {
            bundle: initial_bundle.clone(),
            bundle_for_supersession: initial_bundle,
            capability,
            tier: 1,
            attempts: Vec::new(),
            chain_head: None,
            prior_session: None,
            prior_reason: None,
        })
    }

    fn record_chain_head(&mut self, attempt: &DispatchAttempt) {
        if self.chain_head.is_none() {
            self.chain_head = Some(attempt.session_id.clone());
        }
    }

    fn into_chain_result(self, final_result: WorkerResult) -> ChainResult {
        let head = self
            .chain_head
            .or_else(|| self.attempts.first().map(|a| a.session_id.clone()))
            .unwrap_or_else(|| {
                NamedNode::new_unchecked("https://decision-cli.dev/ns/session/empty")
            });
        ChainResult {
            final_result,
            chain_head: head,
            attempts: self.attempts,
            escalation_exhausted: false,
        }
    }

    fn advance(
        &mut self,
        attempt: DispatchAttempt,
        reason: Option<crate::core::ontology::role_binding::TriggerSignal>,
        new_cap: ResolvedCapability,
    ) {
        let enriched = enrich_bundle_with_prior_attempt(self.bundle.clone(), &attempt, self.tier);
        self.bundle_for_supersession = self.bundle.clone();
        self.bundle = enriched;
        self.prior_session = Some(attempt.session_id);
        self.prior_reason = reason;
        self.capability = new_cap;
        self.tier += 1;
    }
}

fn commit_session(
    writer: &StreamWriter,
    attempt: &DispatchAttempt,
    tokens: AttemptTokens,
    prior_session: Option<&SessionId>,
    prior_reason: Option<crate::core::ontology::role_binding::TriggerSignal>,
) -> Result<(), EscalationError> {
    let mutation = Mutation {
        inserts: build_session_linkage_quads(attempt, tokens, prior_session, prior_reason),
        ..Mutation::default()
    };
    writer
        .commit(mutation)
        .map(|_| ())
        .map_err(|e| EscalationError::GraphWrite(format!("{e:#}")))
}

fn commit_enriched_bundle(
    writer: &StreamWriter,
    bundle: &Bundle,
    attempt: &DispatchAttempt,
    tier: u32,
) -> Result<(), EscalationError> {
    let enriched = enrich_bundle_with_prior_attempt(bundle.clone(), attempt, tier);
    let bundle_quads = build_enriched_bundle_quads(&enriched, bundle);
    writer
        .commit(Mutation {
            inserts: bundle_quads,
            ..Mutation::default()
        })
        .map(|_| ())
        .map_err(|e| EscalationError::GraphWrite(format!("{e:#}")))
}

fn resolve_capability_for_step(
    store: &Store,
    step_iri: &NamedNode,
) -> Result<ResolvedCapability, EscalationError> {
    let cap = query_by_iri(store, step_iri)?.ok_or_else(|| EscalationError::UnknownCapability {
        iri: step_iri.as_str().to_string(),
    })?;
    if cap.status == CapabilityStatus::Eol {
        return Err(EscalationError::CapabilityResolutionFailed {
            iri: step_iri.as_str().to_string(),
            detail: "capability is end-of-life".to_string(),
        });
    }
    Ok(ResolvedCapability {
        capability_id: cap.id.clone(),
        capability_version: cap.version,
        endpoint: cap.endpoint,
        model_identifier: cap.model_identifier.clone(),
        max_output: cap.max_output,
        supports_tool_calling: cap.supports_tool_calling,
        configurable_effort: cap.configurable_effort.unwrap_or(false),
        binding_version: 0,
        cost_cache_hit_per_m: cap.cost_cache_hit_per_m.clone(),
    })
}
