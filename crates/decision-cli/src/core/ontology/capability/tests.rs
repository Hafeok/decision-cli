//! Unit tests for Capability serialisation and SHACL validation.

use oxigraph::model::Literal;

use super::shacl::validate_quads;
use super::types::{Capability, CapabilityStatus, CostCurrency, Endpoint};
use crate::core::vocab::{capability_graph, IRI_DEC_COST_INPUT_PER_M};

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

#[test]
fn well_formed_code_writer_passes_shacl() {
    let cap = code_writer();
    let quads = cap.to_quads(capability_graph());
    validate_quads(&quads).expect("code-writer must pass capability SHACL");
}

#[test]
fn well_formed_anthropic_with_cache_pair_passes_shacl() {
    let cap = deep_reasoning();
    let quads = cap.to_quads(capability_graph());
    validate_quads(&quads).expect("deep-reasoning must pass capability SHACL");
}

#[test]
fn negative_cost_is_rejected() {
    let cap = code_writer();
    let mut quads = cap.to_quads(capability_graph());
    for q in quads.iter_mut() {
        if q.predicate.as_str() == IRI_DEC_COST_INPUT_PER_M {
            q.object = Literal::new_typed_literal(
                "-1.0",
                oxigraph::model::NamedNode::new_unchecked("http://www.w3.org/2001/XMLSchema#decimal"),
            )
            .into();
        }
    }
    let err = validate_quads(&quads).expect_err("negative cost must fail SHACL");
    assert!(err.report.contains("cost_input_per_m"), "{}", err.report);
}

#[test]
fn endpoint_parses_round_trip() {
    for e in [Endpoint::Scaleway, Endpoint::Anthropic] {
        assert_eq!(Endpoint::try_from_str(e.as_str()), Some(e));
    }
    assert_eq!(Endpoint::try_from_str("gcp-vertex"), None);
}

#[test]
fn status_parses_round_trip() {
    for s in [
        CapabilityStatus::Active,
        CapabilityStatus::Preview,
        CapabilityStatus::Eol,
        CapabilityStatus::Candidate,
    ] {
        assert_eq!(CapabilityStatus::try_from_str(s.as_str()), Some(s));
    }
    assert_eq!(CapabilityStatus::try_from_str("retired"), None);
}

#[test]
fn currency_parses_round_trip() {
    for c in [CostCurrency::Eur, CostCurrency::Usd] {
        assert_eq!(CostCurrency::try_from_str(c.as_str()), Some(c));
    }
    assert_eq!(CostCurrency::try_from_str("GBP"), None);
}
