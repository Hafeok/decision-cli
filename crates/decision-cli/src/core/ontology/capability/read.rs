//! SPARQL read API for `dec:Capability` artifacts (FT-054 §Behaviour, step 4).
//!
//! Provides `query_active_by_id` — returns the single active capability
//! whose `dec:capability_id` matches the input, or `None` when no active
//! capability exists for that id. The dispatcher uses this at resolution
//! time per ADR-033.

use oxigraph::model::{NamedNode, Quad, Term};
use oxigraph::sparql::QueryResults;
use oxigraph::store::Store;
use thiserror::Error;

use crate::core::vocab::{
    CAPABILITY_STATUS_ACTIVE, IRI_DEC_BOOTSTRAP_SOURCE, IRI_DEC_CAPABILITY,
    IRI_DEC_CAPABILITY_ENDPOINT, IRI_DEC_CAPABILITY_ID, IRI_DEC_CAPABILITY_NOTES,
    IRI_DEC_CAPABILITY_STATUS, IRI_DEC_CAPABILITY_SUPERSEDES, IRI_DEC_CAPABILITY_VERSION,
    IRI_DEC_CONFIGURABLE_EFFORT, IRI_DEC_CONTEXT_WINDOW, IRI_DEC_COST_CACHE_HIT_PER_M,
    IRI_DEC_COST_CACHE_WRITE_5M, IRI_DEC_COST_CURRENCY, IRI_DEC_COST_INPUT_PER_M,
    IRI_DEC_COST_OUTPUT_PER_M, IRI_DEC_EXPOSES_REASONING_TRACE, IRI_DEC_MAX_OUTPUT,
    IRI_DEC_MODEL_IDENTIFIER, IRI_DEC_SUPPORTS_TOOL_CALLING, IRI_DEC_SUPPORTS_VISION,
    IRI_DEC_TIER,
};

use super::take::{
    format_sparql_string, take_one_bool, take_one_str, take_one_u32, take_optional_bool,
    take_optional_iri, take_optional_str, take_optional_u8,
};
use super::types::{Capability, CapabilityStatus, CostCurrency, Endpoint};

/// Errors produced by [`query_active_by_id`].
#[derive(Debug, Error)]
pub enum CapabilityReadError {
    /// SPARQL query against the store failed.
    #[error("oxigraph store error: {0}")]
    Store(String),
    /// Loaded capability does not match the schema.
    #[error("malformed capability <{iri}>: {detail}")]
    Malformed {
        /// IRI of the offending capability subject.
        iri: String,
        /// Why parsing failed.
        detail: String,
    },
}

/// Return the unique active `dec:Capability` whose `dec:capability_id`
/// matches `id`. Returns `None` when no active capability exists for
/// that id, or an error when more than one is found (a SHACL invariant
/// violation in stored data).
pub fn query_active_by_id(
    store: &Store,
    id: &str,
) -> Result<Option<Capability>, CapabilityReadError> {
    let iris = find_active_iris(store, id)?;
    match iris.len() {
        0 => Ok(None),
        1 => {
            let iri = iris.into_iter().next().expect("len() == 1");
            Ok(Some(load_capability(store, &iri)?))
        }
        n => Err(CapabilityReadError::Malformed {
            iri: format!("dec:capability_id={id:?}"),
            detail: format!(
                "{n} active capabilities share the same capability_id (uniqueness invariant violated)"
            ),
        }),
    }
}

fn find_active_iris(store: &Store, id: &str) -> Result<Vec<NamedNode>, CapabilityReadError> {
    let q = format!(
        "PREFIX dec: <https://decision-cli.dev/ns#> \
         SELECT DISTINCT ?c WHERE {{ \
           {{ ?c a dec:Capability ; \
                  dec:capability_id {id_lit} ; \
                  dec:status \"{active}\" . }} \
           UNION \
           {{ GRAPH ?g {{ ?c a dec:Capability ; \
                              dec:capability_id {id_lit} ; \
                              dec:status \"{active}\" }} }} \
         }}",
        id_lit = format_sparql_string(id),
        active = CAPABILITY_STATUS_ACTIVE,
    );
    let QueryResults::Solutions(sols) = store
        .query(q.as_str())
        .map_err(|e| CapabilityReadError::Store(e.to_string()))?
    else {
        return Ok(Vec::new());
    };
    let mut out: Vec<NamedNode> = Vec::new();
    for sol in sols {
        let sol = sol.map_err(|e| CapabilityReadError::Store(e.to_string()))?;
        if let Some(Term::NamedNode(n)) = sol.get("c") {
            if !out.iter().any(|m| m == n) {
                out.push(n.clone());
            }
        }
    }
    Ok(out)
}

fn load_capability(store: &Store, iri: &NamedNode) -> Result<Capability, CapabilityReadError> {
    let quads = collect_quads(store, iri)?;
    parse_capability(iri.as_str(), &quads)
}

pub(super) fn collect_quads(
    store: &Store,
    iri: &NamedNode,
) -> Result<Vec<Quad>, CapabilityReadError> {
    let mut out: Vec<Quad> = Vec::new();
    let subj = oxigraph::model::Subject::NamedNode(iri.clone());
    for quad in store.quads_for_pattern(Some(subj.as_ref()), None, None, None) {
        let q = quad.map_err(|e| CapabilityReadError::Store(e.to_string()))?;
        out.push(q);
    }
    Ok(out)
}

pub(super) fn parse_capability(
    iri: &str,
    quads: &[Quad],
) -> Result<Capability, CapabilityReadError> {
    let subject = NamedNode::new(iri).map_err(|e| CapabilityReadError::Store(e.to_string()))?;
    require_type(&subject, quads)?;
    let identity = parse_identity(iri, quads, &subject)?;
    let capacity = parse_capacity(iri, quads, &subject)?;
    let pricing = parse_pricing(iri, quads, &subject)?;
    let lifecycle = parse_lifecycle(iri, quads, &subject)?;
    Ok(Capability {
        id: identity.id,
        endpoint: identity.endpoint,
        model_identifier: identity.model_identifier,
        tier: identity.tier,
        context_window: capacity.context_window,
        max_output: capacity.max_output,
        supports_vision: capacity.supports_vision,
        supports_tool_calling: capacity.supports_tool_calling,
        cost_input_per_m: pricing.cost_input_per_m,
        cost_output_per_m: pricing.cost_output_per_m,
        cost_cache_hit_per_m: pricing.cost_cache_hit_per_m,
        cost_cache_write_5m: pricing.cost_cache_write_5m,
        cost_currency: pricing.cost_currency,
        configurable_effort: lifecycle.configurable_effort,
        exposes_reasoning_trace: lifecycle.exposes_reasoning_trace,
        status: lifecycle.status,
        version: lifecycle.version,
        supersedes: lifecycle.supersedes,
        bootstrap_source: lifecycle.bootstrap_source,
        notes: lifecycle.notes,
    })
}

struct Identity {
    id: String,
    endpoint: Endpoint,
    model_identifier: String,
    tier: Option<u8>,
}

struct Capacity {
    context_window: u32,
    max_output: u32,
    supports_vision: bool,
    supports_tool_calling: bool,
}

struct Pricing {
    cost_input_per_m: String,
    cost_output_per_m: String,
    cost_cache_hit_per_m: Option<String>,
    cost_cache_write_5m: Option<String>,
    cost_currency: CostCurrency,
}

struct Lifecycle {
    configurable_effort: Option<bool>,
    exposes_reasoning_trace: Option<bool>,
    status: CapabilityStatus,
    version: u32,
    supersedes: Option<NamedNode>,
    bootstrap_source: Option<String>,
    notes: Option<String>,
}

fn parse_identity(
    iri: &str,
    quads: &[Quad],
    subject: &NamedNode,
) -> Result<Identity, CapabilityReadError> {
    let id = take_one_str(iri, quads, subject, IRI_DEC_CAPABILITY_ID, "dec:capability_id")?;
    let endpoint_raw =
        take_one_str(iri, quads, subject, IRI_DEC_CAPABILITY_ENDPOINT, "dec:endpoint")?;
    let endpoint = Endpoint::try_from_str(&endpoint_raw).ok_or_else(|| {
        CapabilityReadError::Malformed {
            iri: iri.to_string(),
            detail: format!("unknown dec:endpoint value {endpoint_raw:?}"),
        }
    })?;
    let model_identifier = take_one_str(
        iri,
        quads,
        subject,
        IRI_DEC_MODEL_IDENTIFIER,
        "dec:model_identifier",
    )?;
    let tier = take_optional_u8(iri, quads, subject, IRI_DEC_TIER, "dec:tier")?;
    Ok(Identity {
        id,
        endpoint,
        model_identifier,
        tier,
    })
}

fn parse_capacity(
    iri: &str,
    quads: &[Quad],
    subject: &NamedNode,
) -> Result<Capacity, CapabilityReadError> {
    let context_window =
        take_one_u32(iri, quads, subject, IRI_DEC_CONTEXT_WINDOW, "dec:context_window")?;
    let max_output = take_one_u32(iri, quads, subject, IRI_DEC_MAX_OUTPUT, "dec:max_output")?;
    let supports_vision = take_one_bool(
        iri,
        quads,
        subject,
        IRI_DEC_SUPPORTS_VISION,
        "dec:supports_vision",
    )?;
    let supports_tool_calling = take_one_bool(
        iri,
        quads,
        subject,
        IRI_DEC_SUPPORTS_TOOL_CALLING,
        "dec:supports_tool_calling",
    )?;
    Ok(Capacity {
        context_window,
        max_output,
        supports_vision,
        supports_tool_calling,
    })
}

fn parse_pricing(
    iri: &str,
    quads: &[Quad],
    subject: &NamedNode,
) -> Result<Pricing, CapabilityReadError> {
    let cost_input_per_m = take_one_str(
        iri,
        quads,
        subject,
        IRI_DEC_COST_INPUT_PER_M,
        "dec:cost_input_per_m",
    )?;
    let cost_output_per_m = take_one_str(
        iri,
        quads,
        subject,
        IRI_DEC_COST_OUTPUT_PER_M,
        "dec:cost_output_per_m",
    )?;
    let cost_cache_hit_per_m =
        take_optional_str(iri, quads, subject, IRI_DEC_COST_CACHE_HIT_PER_M)?;
    let cost_cache_write_5m =
        take_optional_str(iri, quads, subject, IRI_DEC_COST_CACHE_WRITE_5M)?;
    let cost_currency_raw =
        take_one_str(iri, quads, subject, IRI_DEC_COST_CURRENCY, "dec:cost_currency")?;
    let cost_currency = CostCurrency::try_from_str(&cost_currency_raw).ok_or_else(|| {
        CapabilityReadError::Malformed {
            iri: iri.to_string(),
            detail: format!("unknown dec:cost_currency value {cost_currency_raw:?}"),
        }
    })?;
    Ok(Pricing {
        cost_input_per_m,
        cost_output_per_m,
        cost_cache_hit_per_m,
        cost_cache_write_5m,
        cost_currency,
    })
}

fn parse_lifecycle(
    iri: &str,
    quads: &[Quad],
    subject: &NamedNode,
) -> Result<Lifecycle, CapabilityReadError> {
    let configurable_effort = take_optional_bool(iri, quads, subject, IRI_DEC_CONFIGURABLE_EFFORT)?;
    let exposes_reasoning_trace =
        take_optional_bool(iri, quads, subject, IRI_DEC_EXPOSES_REASONING_TRACE)?;
    let status_raw =
        take_one_str(iri, quads, subject, IRI_DEC_CAPABILITY_STATUS, "dec:status")?;
    let status = CapabilityStatus::try_from_str(&status_raw).ok_or_else(|| {
        CapabilityReadError::Malformed {
            iri: iri.to_string(),
            detail: format!("unknown dec:status value {status_raw:?}"),
        }
    })?;
    let version = take_one_u32(iri, quads, subject, IRI_DEC_CAPABILITY_VERSION, "dec:version")?;
    let supersedes = take_optional_iri(iri, quads, subject, IRI_DEC_CAPABILITY_SUPERSEDES)?;
    let bootstrap_source = take_optional_str(iri, quads, subject, IRI_DEC_BOOTSTRAP_SOURCE)?;
    let notes = take_optional_str(iri, quads, subject, IRI_DEC_CAPABILITY_NOTES)?;
    Ok(Lifecycle {
        configurable_effort,
        exposes_reasoning_trace,
        status,
        version,
        supersedes,
        bootstrap_source,
        notes,
    })
}

fn require_type(subject: &NamedNode, quads: &[Quad]) -> Result<(), CapabilityReadError> {
    let has_type = quads.iter().any(|q| {
        q.predicate.as_str() == "http://www.w3.org/1999/02/22-rdf-syntax-ns#type"
            && matches!(&q.object, Term::NamedNode(n) if n.as_str() == IRI_DEC_CAPABILITY)
            && matches!(&q.subject, oxigraph::model::Subject::NamedNode(s) if s == subject)
    });
    if has_type {
        Ok(())
    } else {
        Err(CapabilityReadError::Malformed {
            iri: subject.as_str().to_string(),
            detail: "missing rdf:type dec:Capability".to_string(),
        })
    }
}

