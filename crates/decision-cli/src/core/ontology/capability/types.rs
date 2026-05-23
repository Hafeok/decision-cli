//! In-memory `dec:Capability` shape + RDF serialisation (FT-054 / ADR-033).

use oxigraph::model::{GraphName, Literal, NamedNode, NamedNodeRef, Quad};

use crate::core::vocab::{
    bootstrap_source_pred, capability_class, capability_endpoint_pred, capability_id_pred,
    capability_notes_pred, capability_status_pred, capability_supersedes_pred,
    capability_version_pred, configurable_effort_pred, context_window_pred, cost_cache_hit_per_m_pred,
    cost_cache_write_5m_pred, cost_currency_pred, cost_input_per_m_pred, cost_output_per_m_pred,
    exposes_reasoning_trace_pred, max_output_pred, model_identifier_pred, supports_tool_calling_pred,
    supports_vision_pred, tier_pred, CAPABILITY_STATUS_ACTIVE, CAPABILITY_STATUS_CANDIDATE,
    CAPABILITY_STATUS_EOL, CAPABILITY_STATUS_PREVIEW, CURRENCY_EUR, CURRENCY_USD, ENDPOINT_ANTHROPIC,
    ENDPOINT_SCALEWAY, IRI_DEC_CAPABILITY_PREFIX,
};

pub(super) const RDF_TYPE: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#type";
pub(super) const XSD_INTEGER: &str = "http://www.w3.org/2001/XMLSchema#integer";
pub(super) const XSD_DECIMAL: &str = "http://www.w3.org/2001/XMLSchema#decimal";
pub(super) const XSD_BOOLEAN: &str = "http://www.w3.org/2001/XMLSchema#boolean";

/// External API endpoint a capability is invoked through (ADR-037).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Endpoint {
    /// Scaleway's OpenAI-compatible inference endpoint.
    Scaleway,
    /// Anthropic Messages API.
    Anthropic,
}

impl Endpoint {
    /// Stable wire string for the endpoint enum.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Scaleway => ENDPOINT_SCALEWAY,
            Self::Anthropic => ENDPOINT_ANTHROPIC,
        }
    }

    /// Parse an endpoint from its wire string. Returns `None` for unknown values.
    #[must_use]
    pub fn try_from_str(s: &str) -> Option<Self> {
        match s {
            ENDPOINT_SCALEWAY => Some(Self::Scaleway),
            ENDPOINT_ANTHROPIC => Some(Self::Anthropic),
            _ => None,
        }
    }
}

/// Lifecycle status of a `dec:Capability` (ADR-033).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CapabilityStatus {
    /// Default — active and resolvable.
    Active,
    /// Preview — not yet stable; dispatcher refuses to bind by default.
    Preview,
    /// End-of-life — dispatcher refuses to resolve.
    Eol,
    /// Candidate — sits in catalog but is not bound to any role yet.
    Candidate,
}

impl CapabilityStatus {
    /// Stable wire string for the status enum.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Active => CAPABILITY_STATUS_ACTIVE,
            Self::Preview => CAPABILITY_STATUS_PREVIEW,
            Self::Eol => CAPABILITY_STATUS_EOL,
            Self::Candidate => CAPABILITY_STATUS_CANDIDATE,
        }
    }

    /// Parse a status from its wire string. Returns `None` for unknown values.
    #[must_use]
    pub fn try_from_str(s: &str) -> Option<Self> {
        match s {
            CAPABILITY_STATUS_ACTIVE => Some(Self::Active),
            CAPABILITY_STATUS_PREVIEW => Some(Self::Preview),
            CAPABILITY_STATUS_EOL => Some(Self::Eol),
            CAPABILITY_STATUS_CANDIDATE => Some(Self::Candidate),
            _ => None,
        }
    }
}

/// Currency unit for a capability's cost fields (ADR-033 §schema, FT-054).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CostCurrency {
    /// Euros — Scaleway invoices.
    Eur,
    /// US Dollars — Anthropic invoices.
    Usd,
}

impl CostCurrency {
    /// Stable wire string for the currency enum.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Eur => CURRENCY_EUR,
            Self::Usd => CURRENCY_USD,
        }
    }

    /// Parse a currency from its wire string. Returns `None` for unknown values.
    #[must_use]
    pub fn try_from_str(s: &str) -> Option<Self> {
        match s {
            CURRENCY_EUR => Some(Self::Eur),
            CURRENCY_USD => Some(Self::Usd),
            _ => None,
        }
    }
}

/// In-memory `dec:Capability` artifact (FT-054).
///
/// Identity is `(capability_id, version)` and the canonical IRI is
/// `https://decision-cli.dev/ns/capability/<id>/v<version>`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Capability {
    /// Stable capability tag used by roles (e.g. `code-writer`).
    pub id: String,
    /// External API endpoint.
    pub endpoint: Endpoint,
    /// Exact provider model string (e.g. `qwen3-coder-30b-a3b-instruct`).
    pub model_identifier: String,
    /// Optional escalation-ladder tier (0..=3); omitted for specialty
    /// and candidate capabilities.
    pub tier: Option<u8>,
    /// Context-window size in tokens (non-negative).
    pub context_window: u32,
    /// Maximum output tokens (non-negative).
    pub max_output: u32,
    /// True iff capability accepts image inputs.
    pub supports_vision: bool,
    /// True iff capability supports function/tool calling.
    pub supports_tool_calling: bool,
    /// Cost per 1M input tokens in `cost_currency` units.
    pub cost_input_per_m: String,
    /// Cost per 1M output tokens in `cost_currency` units.
    pub cost_output_per_m: String,
    /// Optional cost per 1M cache-hit input tokens. Paired with
    /// [`Self::cost_cache_write_5m`] (either both present or both absent).
    pub cost_cache_hit_per_m: Option<String>,
    /// Optional cost per 1M tokens written to the 5-minute TTL cache.
    /// Paired with [`Self::cost_cache_hit_per_m`].
    pub cost_cache_write_5m: Option<String>,
    /// Currency unit for the four cost fields.
    pub cost_currency: CostCurrency,
    /// Optional (default false). True iff capability accepts
    /// `reasoning_effort` parameter (FT-063).
    pub configurable_effort: Option<bool>,
    /// Optional (default false). True iff capability emits a separate
    /// reasoning trace alongside content (FT-060 §10.6).
    pub exposes_reasoning_trace: Option<bool>,
    /// Lifecycle status.
    pub status: CapabilityStatus,
    /// Version (≥1).
    pub version: u32,
    /// Optional link to the prior version this supersedes.
    pub supersedes: Option<NamedNode>,
    /// Optional content hash of the seed YAML this artifact was
    /// bootstrapped from (ADR-036 audit trail).
    pub bootstrap_source: Option<String>,
    /// Optional free-text catalog-maintainer notes.
    pub notes: Option<String>,
}

impl Capability {
    /// Construct the canonical IRI for this capability:
    /// `https://decision-cli.dev/ns/capability/<id>/v<version>`.
    #[must_use]
    pub fn iri(&self) -> NamedNode {
        NamedNode::new_unchecked(format!(
            "{prefix}{id}/v{version}",
            prefix = IRI_DEC_CAPABILITY_PREFIX,
            id = self.id,
            version = self.version,
        ))
    }

    /// Serialise the capability to RDF quads in the supplied named graph.
    #[must_use]
    pub fn to_quads(&self, graph: NamedNodeRef<'_>) -> Vec<Quad> {
        let g: GraphName = graph.into_owned().into();
        let subject = self.iri();
        let mut quads = self.header_quads(&subject, &g);
        quads.extend(self.numeric_quads(&subject, &g));
        quads.extend(self.boolean_quads(&subject, &g));
        quads.extend(self.optional_quads(&subject, &g));
        quads
    }

    fn header_quads(&self, subject: &NamedNode, g: &GraphName) -> Vec<Quad> {
        let rdf_type = NamedNodeRef::new_unchecked(RDF_TYPE);
        let cls = capability_class();
        vec![
            Quad::new(subject.clone(), rdf_type, cls, g.clone()),
            literal_quad(subject, capability_id_pred(), &self.id, g),
            literal_quad(subject, capability_endpoint_pred(), self.endpoint.as_str(), g),
            literal_quad(subject, model_identifier_pred(), &self.model_identifier, g),
            literal_quad(subject, cost_currency_pred(), self.cost_currency.as_str(), g),
            literal_quad(subject, capability_status_pred(), self.status.as_str(), g),
        ]
    }

    fn numeric_quads(&self, subject: &NamedNode, g: &GraphName) -> Vec<Quad> {
        let mut quads = vec![
            typed_literal_quad(
                subject,
                context_window_pred(),
                &self.context_window.to_string(),
                XSD_INTEGER,
                g,
            ),
            typed_literal_quad(
                subject,
                max_output_pred(),
                &self.max_output.to_string(),
                XSD_INTEGER,
                g,
            ),
            typed_literal_quad(
                subject,
                cost_input_per_m_pred(),
                &self.cost_input_per_m,
                XSD_DECIMAL,
                g,
            ),
            typed_literal_quad(
                subject,
                cost_output_per_m_pred(),
                &self.cost_output_per_m,
                XSD_DECIMAL,
                g,
            ),
            typed_literal_quad(
                subject,
                capability_version_pred(),
                &self.version.to_string(),
                XSD_INTEGER,
                g,
            ),
        ];
        if let Some(t) = self.tier {
            quads.push(typed_literal_quad(
                subject,
                tier_pred(),
                &t.to_string(),
                XSD_INTEGER,
                g,
            ));
        }
        if let Some(c) = &self.cost_cache_hit_per_m {
            quads.push(typed_literal_quad(
                subject,
                cost_cache_hit_per_m_pred(),
                c,
                XSD_DECIMAL,
                g,
            ));
        }
        if let Some(c) = &self.cost_cache_write_5m {
            quads.push(typed_literal_quad(
                subject,
                cost_cache_write_5m_pred(),
                c,
                XSD_DECIMAL,
                g,
            ));
        }
        quads
    }

    fn boolean_quads(&self, subject: &NamedNode, g: &GraphName) -> Vec<Quad> {
        let mut quads = vec![
            typed_literal_quad(
                subject,
                supports_vision_pred(),
                &self.supports_vision.to_string(),
                XSD_BOOLEAN,
                g,
            ),
            typed_literal_quad(
                subject,
                supports_tool_calling_pred(),
                &self.supports_tool_calling.to_string(),
                XSD_BOOLEAN,
                g,
            ),
        ];
        if let Some(b) = self.configurable_effort {
            quads.push(typed_literal_quad(
                subject,
                configurable_effort_pred(),
                &b.to_string(),
                XSD_BOOLEAN,
                g,
            ));
        }
        if let Some(b) = self.exposes_reasoning_trace {
            quads.push(typed_literal_quad(
                subject,
                exposes_reasoning_trace_pred(),
                &b.to_string(),
                XSD_BOOLEAN,
                g,
            ));
        }
        quads
    }

    fn optional_quads(&self, subject: &NamedNode, g: &GraphName) -> Vec<Quad> {
        let mut quads = Vec::new();
        if let Some(s) = &self.supersedes {
            quads.push(named_quad(subject, capability_supersedes_pred(), s, g));
        }
        if let Some(s) = &self.bootstrap_source {
            quads.push(literal_quad(subject, bootstrap_source_pred(), s, g));
        }
        if let Some(s) = &self.notes {
            quads.push(literal_quad(subject, capability_notes_pred(), s, g));
        }
        quads
    }
}

pub(super) fn literal_quad(s: &NamedNode, p: NamedNodeRef<'_>, value: &str, g: &GraphName) -> Quad {
    Quad::new(
        s.clone(),
        p.into_owned(),
        Literal::new_simple_literal(value),
        g.clone(),
    )
}

pub(super) fn typed_literal_quad(
    s: &NamedNode,
    p: NamedNodeRef<'_>,
    value: &str,
    datatype: &str,
    g: &GraphName,
) -> Quad {
    Quad::new(
        s.clone(),
        p.into_owned(),
        Literal::new_typed_literal(value, NamedNode::new_unchecked(datatype)),
        g.clone(),
    )
}

pub(super) fn named_quad(s: &NamedNode, p: NamedNodeRef<'_>, o: &NamedNode, g: &GraphName) -> Quad {
    Quad::new(s.clone(), p.into_owned(), o.clone(), g.clone())
}
