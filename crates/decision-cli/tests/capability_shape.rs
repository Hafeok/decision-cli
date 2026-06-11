//! TC-100 — Capability artifact conforms to dec:CapabilityShape SHACL.
//!
//! Validates: FT-054 · ADR-033.
//! Spec: `.product/tests/TC-100-capability-artifact-conforms-to-dec-capabilityshap.md`
//!
//! Two layers of validation are exercised:
//!
//! 1. Every PRD §5.2 capability (built in pure Rust via the
//!    `Capability` struct) is serialised to quads and validated against
//!    the FT-054 SHACL module (`core::ontology::capability::validate_quads`).
//! 2. A battery of constructed-invalid capabilities asserts each
//!    specific SHACL constraint fires with the expected substring.

use std::sync::Arc;

use decision_cli::core::ontology::capability::{
    validate_quads, Capability, CapabilityStatus, CostCurrency, Endpoint,
};
use decision_cli::vocab::{
    capability_endpoint_pred, capability_graph, capability_status_pred, cost_currency_pred,
    cost_input_per_m_pred, IRI_DEC_COST_CACHE_WRITE_5M,
};
use decision_cli::StreamWriter;
use oxi_events::Mutation;
use oxigraph::model::{Literal, NamedNode, Quad, Term};
use oxigraph::store::Store;

const STREAM_IRI: &str = "https://decision-cli.dev/stream/tc-100";

// --- PRD §5.2 catalog -------------------------------------------------------

fn classifier() -> Capability {
    Capability {
        id: "classifier".to_string(),
        endpoint: Endpoint::Scaleway,
        model_identifier: "mistral-small-3.2-24b-instruct-2506".to_string(),
        tier: Some(0),
        context_window: 128_000,
        max_output: 8_192,
        supports_vision: false,
        supports_tool_calling: true,
        cost_input_per_m: "0.15".to_string(),
        cost_output_per_m: "0.35".to_string(),
        cost_cache_hit_per_m: None,
        cost_cache_write_5m: None,
        cost_currency: CostCurrency::Eur,
        configurable_effort: Some(false),
        exposes_reasoning_trace: Some(false),
        status: CapabilityStatus::Active,
        version: 1,
        supersedes: None,
        bootstrap_source: None,
        notes: None,
    }
}

fn code_writer() -> Capability {
    Capability {
        id: "code-writer".to_string(),
        endpoint: Endpoint::Scaleway,
        model_identifier: "qwen3-coder-30b-a3b-instruct".to_string(),
        tier: Some(1),
        context_window: 131_072,
        max_output: 16_384,
        supports_vision: false,
        supports_tool_calling: true,
        cost_input_per_m: "0.20".to_string(),
        cost_output_per_m: "0.80".to_string(),
        cost_cache_hit_per_m: None,
        cost_cache_write_5m: None,
        cost_currency: CostCurrency::Eur,
        configurable_effort: Some(false),
        exposes_reasoning_trace: Some(false),
        status: CapabilityStatus::Active,
        version: 1,
        supersedes: None,
        bootstrap_source: None,
        notes: None,
    }
}

fn code_writer_heavy() -> Capability {
    Capability {
        id: "code-writer-heavy".to_string(),
        endpoint: Endpoint::Scaleway,
        model_identifier: "devstral-2-123b".to_string(),
        tier: Some(2),
        context_window: 256_000,
        max_output: 16_384,
        supports_vision: false,
        supports_tool_calling: true,
        cost_input_per_m: "0.40".to_string(),
        cost_output_per_m: "2.00".to_string(),
        cost_cache_hit_per_m: None,
        cost_cache_write_5m: None,
        cost_currency: CostCurrency::Eur,
        configurable_effort: Some(false),
        exposes_reasoning_trace: Some(false),
        status: CapabilityStatus::Active,
        version: 1,
        supersedes: None,
        bootstrap_source: None,
        notes: None,
    }
}

fn standard_reasoning() -> Capability {
    Capability {
        id: "standard-reasoning".to_string(),
        endpoint: Endpoint::Scaleway,
        model_identifier: "gpt-oss-120b".to_string(),
        tier: Some(1),
        context_window: 131_072,
        max_output: 16_384,
        supports_vision: false,
        supports_tool_calling: true,
        cost_input_per_m: "0.35".to_string(),
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
    Capability {
        id: "standard-reasoning-frontier".to_string(),
        endpoint: Endpoint::Scaleway,
        model_identifier: "qwen3.5-397b-a17b".to_string(),
        tier: Some(2),
        context_window: 256_000,
        max_output: 32_000,
        supports_vision: false,
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

fn mid_reasoning() -> Capability {
    Capability {
        id: "mid-reasoning".to_string(),
        endpoint: Endpoint::Anthropic,
        model_identifier: "claude-sonnet-4-6".to_string(),
        tier: None,
        context_window: 200_000,
        max_output: 16_000,
        supports_vision: true,
        supports_tool_calling: true,
        cost_input_per_m: "3.00".to_string(),
        cost_output_per_m: "15.00".to_string(),
        cost_cache_hit_per_m: Some("0.30".to_string()),
        cost_cache_write_5m: Some("3.75".to_string()),
        cost_currency: CostCurrency::Usd,
        configurable_effort: Some(false),
        exposes_reasoning_trace: Some(false),
        status: CapabilityStatus::Candidate,
        version: 1,
        supersedes: None,
        bootstrap_source: None,
        notes: None,
    }
}

fn fast_reasoning() -> Capability {
    Capability {
        id: "fast-reasoning".to_string(),
        endpoint: Endpoint::Anthropic,
        model_identifier: "claude-haiku-4-5".to_string(),
        tier: None,
        context_window: 200_000,
        max_output: 8_000,
        supports_vision: true,
        supports_tool_calling: true,
        cost_input_per_m: "1.00".to_string(),
        cost_output_per_m: "5.00".to_string(),
        cost_cache_hit_per_m: Some("0.10".to_string()),
        cost_cache_write_5m: Some("1.25".to_string()),
        cost_currency: CostCurrency::Usd,
        configurable_effort: Some(false),
        exposes_reasoning_trace: Some(false),
        status: CapabilityStatus::Candidate,
        version: 1,
        supersedes: None,
        bootstrap_source: None,
        notes: None,
    }
}

fn vision_gui() -> Capability {
    Capability {
        id: "vision-gui".to_string(),
        endpoint: Endpoint::Scaleway,
        model_identifier: "qwen2-vl-72b-instruct".to_string(),
        tier: None,
        context_window: 128_000,
        max_output: 8_192,
        supports_vision: true,
        supports_tool_calling: false,
        cost_input_per_m: "0.50".to_string(),
        cost_output_per_m: "2.00".to_string(),
        cost_cache_hit_per_m: None,
        cost_cache_write_5m: None,
        cost_currency: CostCurrency::Eur,
        configurable_effort: Some(false),
        exposes_reasoning_trace: Some(false),
        status: CapabilityStatus::Active,
        version: 1,
        supersedes: None,
        bootstrap_source: None,
        notes: None,
    }
}

fn vision_general() -> Capability {
    Capability {
        id: "vision-general".to_string(),
        endpoint: Endpoint::Scaleway,
        model_identifier: "pixtral-12b-2409".to_string(),
        tier: None,
        context_window: 128_000,
        max_output: 8_192,
        supports_vision: true,
        supports_tool_calling: false,
        cost_input_per_m: "0.30".to_string(),
        cost_output_per_m: "1.00".to_string(),
        cost_cache_hit_per_m: None,
        cost_cache_write_5m: None,
        cost_currency: CostCurrency::Eur,
        configurable_effort: Some(false),
        exposes_reasoning_trace: Some(false),
        status: CapabilityStatus::Active,
        version: 1,
        supersedes: None,
        bootstrap_source: None,
        notes: None,
    }
}

fn embedding() -> Capability {
    Capability {
        id: "embedding".to_string(),
        endpoint: Endpoint::Scaleway,
        model_identifier: "bge-multilingual-gemma2".to_string(),
        tier: None,
        context_window: 8_192,
        max_output: 0,
        supports_vision: false,
        supports_tool_calling: false,
        cost_input_per_m: "0.05".to_string(),
        cost_output_per_m: "0.00".to_string(),
        cost_cache_hit_per_m: None,
        cost_cache_write_5m: None,
        cost_currency: CostCurrency::Eur,
        configurable_effort: Some(false),
        exposes_reasoning_trace: Some(false),
        status: CapabilityStatus::Active,
        version: 1,
        supersedes: None,
        bootstrap_source: None,
        notes: None,
    }
}

fn audio_transcribe() -> Capability {
    Capability {
        id: "audio-transcribe".to_string(),
        endpoint: Endpoint::Scaleway,
        model_identifier: "whisper-large-v3".to_string(),
        tier: None,
        context_window: 0,
        max_output: 0,
        supports_vision: false,
        supports_tool_calling: false,
        cost_input_per_m: "0.10".to_string(),
        cost_output_per_m: "0.00".to_string(),
        cost_cache_hit_per_m: None,
        cost_cache_write_5m: None,
        cost_currency: CostCurrency::Eur,
        configurable_effort: Some(false),
        exposes_reasoning_trace: Some(false),
        status: CapabilityStatus::Active,
        version: 1,
        supersedes: None,
        bootstrap_source: None,
        notes: None,
    }
}

fn prd_5_2_catalog() -> Vec<Capability> {
    vec![
        classifier(),
        code_writer(),
        code_writer_heavy(),
        standard_reasoning(),
        standard_reasoning_frontier(),
        deep_reasoning(),
        mid_reasoning(),
        fast_reasoning(),
        vision_gui(),
        vision_general(),
        embedding(),
        audio_transcribe(),
    ]
}

fn writer() -> (Arc<Store>, StreamWriter) {
    let store = Arc::new(Store::new().expect("in-memory store"));
    let stream = NamedNode::new(STREAM_IRI).expect("stream iri");
    let w = StreamWriter::bootstrap(Arc::clone(&store), stream).expect("stream writer");
    (store, w)
}

fn commit_quads(w: &StreamWriter, quads: Vec<Quad>) -> Result<(), String> {
    w.commit(Mutation::insert(quads))
        .map(|_| ())
        .map_err(|e| format!("{e:#}"))
}

// --- Pure-SHACL acceptance against the catalog ------------------------------

#[test]
fn every_prd_5_2_capability_passes_shacl() {
    for cap in prd_5_2_catalog() {
        let quads = cap.to_quads(capability_graph());
        validate_quads(&quads)
            .unwrap_or_else(|err| panic!("capability {id:?} must pass SHACL: {err}", id = cap.id));
    }
}

#[test]
fn code_writer_specific_values_pass_shacl() {
    let c = code_writer();
    assert_eq!(c.cost_input_per_m, "0.20");
    assert_eq!(c.cost_output_per_m, "0.80");
    assert_eq!(c.cost_currency, CostCurrency::Eur);
    assert!(c.cost_cache_hit_per_m.is_none());
    assert!(c.cost_cache_write_5m.is_none());
    validate_quads(&c.to_quads(capability_graph())).expect("code-writer passes");
}

#[test]
fn standard_reasoning_has_configurable_effort_true() {
    let c = standard_reasoning();
    assert_eq!(c.configurable_effort, Some(true));
    validate_quads(&c.to_quads(capability_graph())).expect("standard-reasoning passes");
}

#[test]
fn standard_reasoning_frontier_exposes_reasoning_trace() {
    let c = standard_reasoning_frontier();
    assert_eq!(c.exposes_reasoning_trace, Some(true));
    validate_quads(&c.to_quads(capability_graph())).expect("frontier passes");
}

#[test]
fn deep_reasoning_anthropic_has_cache_pair_and_usd() {
    let c = deep_reasoning();
    assert_eq!(c.cost_currency, CostCurrency::Usd);
    assert!(c.cost_cache_hit_per_m.is_some());
    assert!(c.cost_cache_write_5m.is_some());
    validate_quads(&c.to_quads(capability_graph())).expect("deep-reasoning passes");
}

#[test]
fn candidate_status_capabilities_pass_shacl() {
    for c in [mid_reasoning(), fast_reasoning()] {
        assert_eq!(c.status, CapabilityStatus::Candidate);
        validate_quads(&c.to_quads(capability_graph()))
            .unwrap_or_else(|e| panic!("{} must pass SHACL: {e}", c.id));
    }
}

// --- StreamWriter chokepoint acceptance --------------------------------------

#[test]
fn well_formed_capability_commits_through_stream_writer() {
    let (store, w) = writer();
    let c = code_writer();
    let quads = c.to_quads(capability_graph());
    commit_quads(&w, quads).expect("well-formed capability commits");
    let exists = store
        .quads_for_pattern(
            Some(oxigraph::model::Subject::NamedNode(c.iri()).as_ref()),
            None,
            None,
            None,
        )
        .next()
        .is_some();
    assert!(exists, "capability must persist after a successful commit");
}

// --- Battery of invalid capabilities ----------------------------------------

#[test]
fn missing_cost_currency_is_rejected() {
    let (_store, w) = writer();
    let c = code_writer();
    let mut quads = c.to_quads(capability_graph());
    quads.retain(|q| q.predicate.as_str() != cost_currency_pred().as_str());
    let err = commit_quads(&w, quads).expect_err("missing cost_currency must fail");
    assert!(err.contains("SHACL violation"), "{err}");
    assert!(err.contains("cost_currency"), "{err}");
}

#[test]
fn invalid_currency_string_is_rejected() {
    let (_store, w) = writer();
    let c = code_writer();
    let mut quads = c.to_quads(capability_graph());
    for q in quads.iter_mut() {
        if q.predicate.as_str() == cost_currency_pred().as_str() {
            q.object = Literal::new_simple_literal("GBP").into();
        }
    }
    let err = commit_quads(&w, quads).expect_err("currency=GBP must fail");
    assert!(err.contains("SHACL violation"), "{err}");
    assert!(err.contains("cost_currency"), "{err}");
    assert!(
        err.contains("EUR") || err.contains("USD"),
        "detail must list accepted currency: {err}"
    );
}

#[test]
fn invalid_endpoint_is_rejected() {
    let (_store, w) = writer();
    let c = code_writer();
    let mut quads = c.to_quads(capability_graph());
    for q in quads.iter_mut() {
        if q.predicate.as_str() == capability_endpoint_pred().as_str() {
            if matches!(q.object, Term::Literal(_)) {
                q.object = Literal::new_simple_literal("gcp-vertex").into();
            }
        }
    }
    let err = commit_quads(&w, quads).expect_err("unknown endpoint must fail");
    assert!(err.contains("SHACL violation"), "{err}");
    assert!(err.contains("endpoint"), "{err}");
}

#[test]
fn invalid_status_is_rejected() {
    let (_store, w) = writer();
    let c = code_writer();
    let mut quads = c.to_quads(capability_graph());
    for q in quads.iter_mut() {
        if q.predicate.as_str() == capability_status_pred().as_str() {
            q.object = Literal::new_simple_literal("retired").into();
        }
    }
    let err = commit_quads(&w, quads).expect_err("unknown status must fail");
    assert!(err.contains("SHACL violation"), "{err}");
    assert!(err.contains("status"), "{err}");
}

#[test]
fn negative_cost_is_rejected() {
    let (_store, w) = writer();
    let c = code_writer();
    let mut quads = c.to_quads(capability_graph());
    for q in quads.iter_mut() {
        if q.predicate.as_str() == cost_input_per_m_pred().as_str() {
            q.object = Literal::new_typed_literal(
                "-0.5",
                NamedNode::new_unchecked("http://www.w3.org/2001/XMLSchema#decimal"),
            )
            .into();
        }
    }
    let err = commit_quads(&w, quads).expect_err("negative cost must fail");
    assert!(err.contains("SHACL violation"), "{err}");
    assert!(err.contains("cost_input_per_m"), "{err}");
}

#[test]
fn duplicate_active_id_and_version_is_rejected() {
    let (_store, w) = writer();
    // Build two distinct capability artifacts that share (capability_id, version) and status=active.
    let mut a = code_writer();
    a.notes = Some("first".to_string());
    let mut b = code_writer();
    b.notes = Some("second".to_string());
    // Same iri() makes the second commit fold into the first; force distinct IRIs
    // by overriding via a second capability with same id+version but different model_identifier.
    // Per FT-054 the uniqueness is over the (capability_id, version) tuple, not the IRI; the
    // SHACL constraint must catch this regardless of IRI shape.
    let mut quads_a = a.to_quads(capability_graph());
    let mut quads_b = b.to_quads(capability_graph());
    // Distinct subject IRIs to genuinely exercise the cross-subject constraint.
    let new_subject =
        NamedNode::new_unchecked("https://decision-cli.dev/ns/capability/code-writer/v1#alt");
    for q in quads_b.iter_mut() {
        let oxigraph::model::Subject::NamedNode(_) = &q.subject else {
            continue;
        };
        q.subject = oxigraph::model::Subject::NamedNode(new_subject.clone());
    }
    quads_a.extend(quads_b);
    let err = commit_quads(&w, quads_a)
        .expect_err("duplicate (capability_id, version) among active must fail");
    assert!(err.contains("SHACL violation"), "{err}");
    assert!(err.contains("duplicated"), "{err}");
}

#[test]
fn tier_above_three_is_rejected() {
    let (_store, w) = writer();
    let mut c = code_writer();
    c.tier = Some(7);
    let quads = c.to_quads(capability_graph());
    let err = commit_quads(&w, quads).expect_err("tier=7 must fail");
    assert!(err.contains("SHACL violation"), "{err}");
    assert!(err.contains("tier"), "{err}");
}

#[test]
fn cache_hit_without_cache_write_is_rejected() {
    let (_store, w) = writer();
    let mut c = code_writer();
    c.cost_cache_hit_per_m = Some("0.50".to_string());
    c.cost_cache_write_5m = None;
    let quads = c.to_quads(capability_graph());
    let err = commit_quads(&w, quads).expect_err("hit without write must fail");
    assert!(err.contains("SHACL violation"), "{err}");
    assert!(err.contains("paired"), "{err}");
}

#[test]
fn cache_write_without_cache_hit_is_rejected() {
    let (_store, w) = writer();
    let c = deep_reasoning();
    let mut quads = c.to_quads(capability_graph());
    // Drop the cache_hit predicate but keep cache_write_5m.
    quads.retain(|q| q.predicate.as_str() != "https://decision-cli.dev/ns#cost_cache_hit_per_m");
    // Sanity check: cache_write_5m is still present.
    assert!(quads
        .iter()
        .any(|q| q.predicate.as_str() == IRI_DEC_COST_CACHE_WRITE_5M));
    let err = commit_quads(&w, quads).expect_err("write without hit must fail");
    assert!(err.contains("SHACL violation"), "{err}");
    assert!(err.contains("paired"), "{err}");
}

#[test]
fn embedded_shapes_declare_capability_shape() {
    use decision_cli::OntologyHandle;
    let h = OntologyHandle::load().expect("load ontology");
    let target = NamedNode::new("https://decision-cli.dev/ns#Capability").expect("class iri");
    let mut has_shape = false;
    for q in h.shapes_graph() {
        if q.predicate.as_str() == "http://www.w3.org/ns/shacl#targetClass" {
            if let Term::NamedNode(n) = &q.object {
                if n == &target {
                    has_shape = true;
                    break;
                }
            }
        }
    }
    assert!(
        has_shape,
        "shapes.ttl must declare a sh:NodeShape with sh:targetClass dec:Capability"
    );
}
