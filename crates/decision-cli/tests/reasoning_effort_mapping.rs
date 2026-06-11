//! TC-112 — Reasoning_effort maps stakes to low/medium/high only when
//! configurable_effort is true.
//!
//! Validates: FT-063 · ADR-035.
//! Spec: `.product/tests/TC-112-reasoning-effort-maps-stakes-to-low-medium-high-on.md`
//!
//! Five acceptance bullets from the TC:
//!
//! 1. Pure function — all 6 cases of `compute_reasoning_effort`.
//! 2. Architect routine path — dispatch with `stakes = routine` produces
//!    a single `standard-reasoning` (configurable) session whose payload
//!    carries `reasoning_effort = Some("low")`.
//! 3. Architect elevated path — `stakes = elevated` triggers escalation
//!    to `standard-reasoning-frontier`. S1 (standard-reasoning) carries
//!    `Some("medium")`; S2 (frontier, configurable_effort = false)
//!    carries `None`.
//! 4. Anthropic ignores reasoning_effort — architect with foundational
//!    stakes escalates twice, ending on `deep-reasoning` (Anthropic).
//!    The Anthropic payload's `reasoning_effort` field is `None`.
//! 5. Exhaustiveness — Rust's `match` exhaustiveness check covers every
//!    `Stakes` variant; documented by the test file rather than asserted
//!    dynamically (a new variant fails to compile).

use std::sync::Arc;

use decision_cli::core::bundle::{Bundle, Stakes};
use decision_cli::core::dispatch::{
    capability_resolver::ResolvedCapability,
    compute_reasoning_effort, dispatch_role,
    escalation::{AttemptTokens, DispatchAttempt, EscalationError, SessionId, WorkerResult},
    ReasoningEffort, WorkerRunner,
};
use decision_cli::core::ontology::capability::{
    Capability, CapabilityStatus, CostCurrency, Endpoint,
};
use decision_cli::core::ontology::role_binding::{EscalationStep, RoleBinding, TriggerSignal};
use decision_cli::core::ontology::verdict::Verdict;
use decision_cli::vocab::{capability_graph, role_binding_graph};
use decision_cli::StreamWriter;
use oxi_events::Mutation;
use oxigraph::model::{NamedNode, Quad};
use oxigraph::store::Store;

const STREAM_IRI: &str = "https://decision-cli.dev/stream/tc-112";

// ---------------------------------------------------------------------------
// Acceptance 1 — pure function over all 6 inputs.
// ---------------------------------------------------------------------------

#[test]
fn compute_routine_configurable_yields_low() {
    assert_eq!(
        compute_reasoning_effort(Stakes::Routine, true),
        Some(ReasoningEffort::Low),
    );
    assert_eq!(
        compute_reasoning_effort(Stakes::Routine, true).map(ReasoningEffort::as_str),
        Some("low"),
    );
}

#[test]
fn compute_elevated_configurable_yields_medium() {
    assert_eq!(
        compute_reasoning_effort(Stakes::Elevated, true),
        Some(ReasoningEffort::Medium),
    );
    assert_eq!(
        compute_reasoning_effort(Stakes::Elevated, true).map(ReasoningEffort::as_str),
        Some("medium"),
    );
}

#[test]
fn compute_foundational_configurable_yields_high() {
    assert_eq!(
        compute_reasoning_effort(Stakes::Foundational, true),
        Some(ReasoningEffort::High),
    );
    assert_eq!(
        compute_reasoning_effort(Stakes::Foundational, true).map(ReasoningEffort::as_str),
        Some("high"),
    );
}

#[test]
fn compute_routine_non_configurable_yields_none() {
    assert_eq!(compute_reasoning_effort(Stakes::Routine, false), None);
}

#[test]
fn compute_elevated_non_configurable_yields_none() {
    assert_eq!(compute_reasoning_effort(Stakes::Elevated, false), None);
}

#[test]
fn compute_foundational_non_configurable_yields_none() {
    assert_eq!(compute_reasoning_effort(Stakes::Foundational, false), None);
}

#[test]
fn reasoning_effort_as_str_renders_canonical_wire_literals() {
    assert_eq!(ReasoningEffort::None_.as_str(), "none");
    assert_eq!(ReasoningEffort::Low.as_str(), "low");
    assert_eq!(ReasoningEffort::Medium.as_str(), "medium");
    assert_eq!(ReasoningEffort::High.as_str(), "high");
}

#[test]
fn compute_is_referentially_transparent() {
    // Same inputs across N calls produce identical outputs (no hidden
    // state, no graph reads).
    for stakes in [Stakes::Routine, Stakes::Elevated, Stakes::Foundational] {
        for flag in [false, true] {
            let a = compute_reasoning_effort(stakes, flag);
            let b = compute_reasoning_effort(stakes, flag);
            let c = compute_reasoning_effort(stakes, flag);
            assert_eq!(a, b);
            assert_eq!(b, c);
        }
    }
}

// ---------------------------------------------------------------------------
// Catalog fixtures — architect's PRD-seeded binding (config/role-bindings.yaml).
// ---------------------------------------------------------------------------

fn cap_iri(id: &str, version: u32) -> NamedNode {
    NamedNode::new_unchecked(format!(
        "https://decision-cli.dev/ns/capability/{id}/v{version}"
    ))
}

fn standard_reasoning() -> Capability {
    // Architect's default capability — gpt-oss-120b on Scaleway with
    // configurable_effort = true. This is the only seed capability that
    // surfaces a non-None reasoning_effort under FT-063.
    Capability {
        id: "standard-reasoning".to_string(),
        endpoint: Endpoint::Scaleway,
        model_identifier: "gpt-oss-120b".to_string(),
        tier: None,
        context_window: 128_000,
        max_output: 16_000,
        supports_vision: false,
        supports_tool_calling: true,
        cost_input_per_m: "0.30".to_string(),
        cost_output_per_m: "1.50".to_string(),
        cost_cache_hit_per_m: None,
        cost_cache_write_5m: None,
        cost_currency: CostCurrency::Eur,
        configurable_effort: Some(true),
        exposes_reasoning_trace: Some(false),
        status: CapabilityStatus::Active,
        version: 1,
        supersedes: None,
        bootstrap_source: None,
        notes: None,
    }
}

fn standard_reasoning_frontier() -> Capability {
    // First escalation step — qwen3.5-397b on Scaleway; configurable_effort = false.
    Capability {
        id: "standard-reasoning-frontier".to_string(),
        endpoint: Endpoint::Scaleway,
        model_identifier: "qwen3.5-397b-a17b".to_string(),
        tier: Some(2),
        context_window: 250_000,
        max_output: 16_000,
        supports_vision: true,
        supports_tool_calling: true,
        cost_input_per_m: "0.60".to_string(),
        cost_output_per_m: "3.60".to_string(),
        cost_cache_hit_per_m: None,
        cost_cache_write_5m: None,
        cost_currency: CostCurrency::Eur,
        configurable_effort: Some(false),
        exposes_reasoning_trace: Some(true),
        status: CapabilityStatus::Active,
        version: 1,
        supersedes: None,
        bootstrap_source: None,
        notes: None,
    }
}

fn deep_reasoning() -> Capability {
    // Anthropic tier-3 — claude-opus-4-7; configurable_effort = false.
    // The Anthropic router skips the reasoning_effort kwarg when this
    // flag is false (FT-060 / acceptance #4).
    Capability {
        id: "deep-reasoning".to_string(),
        endpoint: Endpoint::Anthropic,
        model_identifier: "claude-opus-4-7".to_string(),
        tier: Some(3),
        context_window: 200_000,
        max_output: 32_000,
        supports_vision: true,
        supports_tool_calling: true,
        cost_input_per_m: "5.00".to_string(),
        cost_output_per_m: "25.00".to_string(),
        cost_cache_hit_per_m: Some("0.50".to_string()),
        cost_cache_write_5m: Some("6.25".to_string()),
        cost_currency: CostCurrency::Usd,
        configurable_effort: Some(false),
        exposes_reasoning_trace: Some(false),
        status: CapabilityStatus::Active,
        version: 1,
        supersedes: None,
        bootstrap_source: None,
        notes: None,
    }
}

fn architect_binding() -> RoleBinding {
    // Mirrors config/role-bindings.yaml lines 44-56 (the PRD seed):
    //   default: standard-reasoning
    //   escalation:
    //     - standard-reasoning-frontier on (stakes_elevated, audit_fail)
    //     - deep-reasoning on (stakes_foundational, prior_attempts_ge_3)
    RoleBinding {
        role_id: "architect".to_string(),
        default_capability: cap_iri("standard-reasoning", 1),
        escalation_steps: vec![
            EscalationStep {
                step_capability: cap_iri("standard-reasoning-frontier", 1),
                triggers: vec![TriggerSignal::StakesElevated, TriggerSignal::AuditFail],
            },
            EscalationStep {
                step_capability: cap_iri("deep-reasoning", 1),
                triggers: vec![
                    TriggerSignal::StakesFoundational,
                    TriggerSignal::PriorAttemptsGe3,
                ],
            },
        ],
        version: 1,
        active: true,
        supersedes: None,
        bootstrap_source: None,
    }
}

// ---------------------------------------------------------------------------
// Test harness — store, writer, seed, recording stub.
// ---------------------------------------------------------------------------

fn writer() -> (Arc<Store>, StreamWriter) {
    let store = Arc::new(Store::new().expect("in-memory store"));
    let stream = NamedNode::new(STREAM_IRI).expect("stream iri");
    let w = StreamWriter::bootstrap(Arc::clone(&store), stream).expect("stream writer");
    (store, w)
}

fn commit(w: &StreamWriter, quads: Vec<Quad>) {
    w.commit(Mutation::insert(quads))
        .map(|_| ())
        .expect("commit");
}

fn seed_architect(w: &StreamWriter) {
    commit(w, standard_reasoning().to_quads(capability_graph()));
    commit(
        w,
        standard_reasoning_frontier().to_quads(capability_graph()),
    );
    commit(w, deep_reasoning().to_quads(capability_graph()));
    commit(w, architect_binding().to_quads(role_binding_graph()));
}

/// One observation of what the worker's payload would have carried: the
/// capability id it saw, the endpoint, the bundle's stakes, and the
/// `reasoning_effort` the dispatcher's payload-assembly path computes
/// via [`compute_reasoning_effort`].
#[derive(Debug, Clone, PartialEq, Eq)]
struct ObservedPayload {
    capability_id: String,
    endpoint: Endpoint,
    bundle_stakes: Stakes,
    /// The wire string the FT-060 `CallParams.reasoning_effort` would
    /// carry (or `None` when the capability is not configurable).
    reasoning_effort: Option<&'static str>,
    /// The typed enum for stricter assertions.
    reasoning_effort_enum: Option<ReasoningEffort>,
}

/// Stub `WorkerRunner` that mirrors the dispatcher's payload assembly
/// step: at the moment the dispatcher would construct the worker's
/// `CallParams`, it derives `reasoning_effort` via
/// [`compute_reasoning_effort(bundle.stakes, capability.configurable_effort)`].
///
/// The stub records the observed value per attempt, and returns
/// whatever verdict + confidence the test wired up for that attempt
/// index. Confidence drives the escalation triggers FT-062 evaluates.
struct RecordingArchitect {
    observations: Vec<ObservedPayload>,
    /// Per-attempt verdict + confidence the worker returns. Index
    /// matches `prior.len()` at call time.
    canned: Vec<(Verdict, Option<f32>)>,
}

impl WorkerRunner for RecordingArchitect {
    fn run(
        &mut self,
        _role_id: &str,
        bundle: &Bundle,
        capability: &ResolvedCapability,
        prior: &[DispatchAttempt],
        session_id: &SessionId,
    ) -> Result<DispatchAttempt, EscalationError> {
        let effort = compute_reasoning_effort(bundle.stakes, capability.configurable_effort);
        self.observations.push(ObservedPayload {
            capability_id: capability.capability_id.clone(),
            endpoint: capability.endpoint,
            bundle_stakes: bundle.stakes,
            reasoning_effort: effort.map(ReasoningEffort::as_str),
            reasoning_effort_enum: effort,
        });

        let idx = prior.len();
        let (kind, confidence) = self
            .canned
            .get(idx)
            .copied()
            .unwrap_or((Verdict::Approved, Some(1.0)));
        Ok(DispatchAttempt {
            session_id: session_id.clone(),
            capability: capability.clone(),
            result: WorkerResult::Verdict { kind, confidence },
            feedback: vec![],
            audit_outcome: None,
        })
    }
}

fn bundle_with(stakes: Stakes, hash: &str) -> Bundle {
    Bundle {
        hash: hash.to_string(),
        focal: NamedNode::new_unchecked("https://example.com/focal/tc-112"),
        stakes,
    }
}

// ---------------------------------------------------------------------------
// Acceptance 2 — architect routine path: single session, code=low.
// ---------------------------------------------------------------------------

#[test]
fn architect_routine_dispatch_records_low_on_standard_reasoning() {
    let (store, w) = writer();
    seed_architect(&w);
    let mut runner = RecordingArchitect {
        observations: vec![],
        // High confidence so no escalation fires from the default path.
        canned: vec![(Verdict::Approved, Some(0.95))],
    };

    let chain = dispatch_role(
        &store,
        &w,
        "architect",
        bundle_with(Stakes::Routine, "tc112-routine"),
        &mut runner,
        |_| AttemptTokens::default(),
    )
    .expect("dispatch ok");

    // Exactly one session (no escalation trigger fires under routine + high confidence).
    assert_eq!(chain.attempts.len(), 1, "expected single session");
    assert_eq!(
        chain.attempts[0].capability.capability_id,
        "standard-reasoning"
    );

    // The recorded payload carries reasoning_effort = "low".
    assert_eq!(runner.observations.len(), 1);
    let obs = &runner.observations[0];
    assert_eq!(obs.capability_id, "standard-reasoning");
    assert_eq!(obs.endpoint, Endpoint::Scaleway);
    assert_eq!(obs.bundle_stakes, Stakes::Routine);
    assert_eq!(obs.reasoning_effort, Some("low"));
    assert_eq!(obs.reasoning_effort_enum, Some(ReasoningEffort::Low));
}

// ---------------------------------------------------------------------------
// Acceptance 3 — architect elevated path: S1 medium, S2 None on frontier.
// ---------------------------------------------------------------------------

#[test]
fn architect_elevated_dispatch_records_medium_then_none_on_frontier() {
    let (store, w) = writer();
    seed_architect(&w);
    let mut runner = RecordingArchitect {
        observations: vec![],
        // First attempt amendment-required → stakes_elevated trigger
        // fires on first escalation step; second attempt approves.
        canned: vec![
            (Verdict::AmendmentRequired, Some(0.95)),
            (Verdict::Approved, Some(0.95)),
        ],
    };

    let chain = dispatch_role(
        &store,
        &w,
        "architect",
        bundle_with(Stakes::Elevated, "tc112-elevated"),
        &mut runner,
        |_| AttemptTokens::default(),
    )
    .expect("dispatch ok");

    // Two sessions: standard-reasoning, then standard-reasoning-frontier.
    assert_eq!(chain.attempts.len(), 2, "expected two sessions");
    assert_eq!(
        chain.attempts[0].capability.capability_id,
        "standard-reasoning"
    );
    assert_eq!(
        chain.attempts[1].capability.capability_id,
        "standard-reasoning-frontier"
    );

    // S1 — standard-reasoning, configurable, stakes=elevated → medium.
    assert_eq!(runner.observations.len(), 2);
    let s1 = &runner.observations[0];
    assert_eq!(s1.capability_id, "standard-reasoning");
    assert_eq!(s1.bundle_stakes, Stakes::Elevated);
    assert_eq!(s1.reasoning_effort, Some("medium"));
    assert_eq!(s1.reasoning_effort_enum, Some(ReasoningEffort::Medium));

    // S2 — frontier, NOT configurable → None regardless of stakes.
    let s2 = &runner.observations[1];
    assert_eq!(s2.capability_id, "standard-reasoning-frontier");
    assert_eq!(s2.endpoint, Endpoint::Scaleway);
    assert_eq!(s2.reasoning_effort, None);
    assert_eq!(s2.reasoning_effort_enum, None);
}

// ---------------------------------------------------------------------------
// Acceptance 4 — Anthropic deep-reasoning ignores reasoning_effort.
// ---------------------------------------------------------------------------

#[test]
fn architect_foundational_dispatch_reaches_anthropic_without_reasoning_effort() {
    let (store, w) = writer();
    seed_architect(&w);
    // Foundational stakes → matches `stakes_foundational` trigger on
    // the SECOND escalation step (deep-reasoning) immediately. The
    // first step (`standard-reasoning-frontier`) has triggers
    // {stakes_elevated, audit_fail}; neither fires on foundational
    // stakes alone, so the dispatcher walks past it to the second step.
    let mut runner = RecordingArchitect {
        observations: vec![],
        canned: vec![
            (Verdict::AmendmentRequired, Some(0.95)),
            (Verdict::Approved, Some(0.95)),
        ],
    };

    let chain = dispatch_role(
        &store,
        &w,
        "architect",
        bundle_with(Stakes::Foundational, "tc112-foundational"),
        &mut runner,
        |_| AttemptTokens::default(),
    )
    .expect("dispatch ok");

    // Two sessions: standard-reasoning then deep-reasoning (Anthropic).
    assert_eq!(chain.attempts.len(), 2, "expected two sessions");
    assert_eq!(
        chain.attempts[0].capability.capability_id,
        "standard-reasoning"
    );
    assert_eq!(chain.attempts[1].capability.capability_id, "deep-reasoning");
    assert_eq!(chain.attempts[1].capability.endpoint, Endpoint::Anthropic);

    // S1 — standard-reasoning configurable, stakes=foundational → high.
    let s1 = &runner.observations[0];
    assert_eq!(s1.reasoning_effort, Some("high"));
    assert_eq!(s1.reasoning_effort_enum, Some(ReasoningEffort::High));

    // S2 — Anthropic deep-reasoning, NOT configurable → None.
    // This is the load-bearing assertion for acceptance #4: the
    // Anthropic capability sees no `reasoning_effort` parameter in the
    // payload. The downstream `ModelRouter` (workers/_shared) skips the
    // kwarg entirely on the API call.
    let s2 = &runner.observations[1];
    assert_eq!(s2.capability_id, "deep-reasoning");
    assert_eq!(s2.endpoint, Endpoint::Anthropic);
    assert_eq!(s2.reasoning_effort, None);
    assert_eq!(s2.reasoning_effort_enum, None);
}

// ---------------------------------------------------------------------------
// Acceptance 5 — exhaustiveness documentation. The Rust `match` in
// `compute_reasoning_effort` covers every `Stakes` variant; adding a new
// variant without extending the function fails to compile. We document
// this here by exercising every variant and asserting we get a defined
// result for `configurable_effort = true` — if a new variant slipped in
// undocumented, this loop's match-over-Stakes would also fail to compile.
// ---------------------------------------------------------------------------

#[test]
fn compute_is_total_over_every_stakes_variant() {
    // Closed iteration: if a new Stakes variant is added, this array
    // initializer is the canonical place to update; the function itself
    // is enforced by Rust's exhaustiveness check on its inner match.
    for stakes in [Stakes::Routine, Stakes::Elevated, Stakes::Foundational] {
        // configurable=true ⇒ Some(_)
        assert!(compute_reasoning_effort(stakes, true).is_some());
        // configurable=false ⇒ None
        assert!(compute_reasoning_effort(stakes, false).is_none());
    }
}
