//! Authority declarations per ADR-027 — bounded role authority.
//!
//! Every `dec:Role` carries exactly one `dec:Authority` linking to a
//! node that enumerates which categories the role's worker MAY decide
//! unilaterally and which it MUST escalate as feedback. The vocabulary
//! is graph-resident, queryable, and reviewable. FT-030 lands the
//! Phase A baseline (implementer + verifier).
//!
//! Per the slice-level SDP convention in `CLAUDE.md`, every consumer
//! (FT-031 worker SDK, slice-3 feedback routing) reads authority data
//! through this module — never through a sibling feature.

use anyhow::{anyhow, Context, Result};
use oxigraph::sparql::QueryResults;
use oxigraph::store::Store;

use crate::core::feedback::FeedbackClass;

/// `dec:Authority` IRI.
pub const AUTHORITY_CLASS_IRI: &str = "https://decision-cli.dev/ns#Authority";

/// `dec:authority` (Role → Authority) IRI.
pub const AUTHORITY_PRED_IRI: &str = "https://decision-cli.dev/ns#authority";

/// `dec:mayDecide` IRI.
pub const MAY_DECIDE_IRI: &str = "https://decision-cli.dev/ns#mayDecide";

/// `dec:mustEscalate` IRI.
pub const MUST_ESCALATE_IRI: &str = "https://decision-cli.dev/ns#mustEscalate";

/// `dec:escalateVia` IRI.
pub const ESCALATE_VIA_IRI: &str = "https://decision-cli.dev/ns#escalateVia";

/// `dec:escalateCategory` IRI.
pub const ESCALATE_CATEGORY_IRI: &str = "https://decision-cli.dev/ns#escalateCategory";

/// `dec:escalateClass` IRI.
pub const ESCALATE_CLASS_IRI: &str = "https://decision-cli.dev/ns#escalateClass";

/// `dec:escalateTargetRole` IRI.
pub const ESCALATE_TARGET_ROLE_IRI: &str = "https://decision-cli.dev/ns#escalateTargetRole";

/// `dec:rationale` IRI (shared with VerificationVerdict; on Authority
/// it carries the human-readable boundary explanation).
pub const RATIONALE_IRI: &str = "https://decision-cli.dev/ns#rationale";

/// Stable IRI minted for the implementer authority blank-node seed.
pub const IMPLEMENTER_AUTHORITY_IRI: &str = "https://decision-cli.dev/ns/authority/implementer";

/// Stable IRI minted for the verifier authority blank-node seed.
pub const VERIFIER_AUTHORITY_IRI: &str = "https://decision-cli.dev/ns/authority/verifier";

/// Per-mustEscalate-category routing hint (ADR-027).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EscalationHint {
    /// The category name this hint applies to (matches a `mustEscalate` value).
    pub category: String,
    /// Feedback class the escalation should be emitted as.
    pub class: FeedbackClass,
    /// Default target role for the escalation.
    pub target_role: String,
}

/// A role's bounded authority declaration (ADR-027 / FT-030).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Authority {
    /// IRI of the `dec:Authority` artifact.
    pub iri: String,
    /// Categories the worker MAY decide unilaterally.
    pub may_decide: Vec<String>,
    /// Categories the worker MUST escalate via feedback.
    pub must_escalate: Vec<String>,
    /// Per-category routing hints.
    pub escalate_via: Vec<EscalationHint>,
    /// Free-form prose explaining the boundary.
    pub rationale: String,
}

impl Authority {
    /// Read the authority node at `iri` from the store and return it as
    /// a structured `Authority`. Returns `Ok(None)` when the IRI has no
    /// `dec:Authority` triples in the store.
    pub fn load(store: &Store, iri: &str) -> Result<Option<Self>> {
        let may_decide = collect_literal_property(store, iri, MAY_DECIDE_IRI)?;
        let must_escalate = collect_literal_property(store, iri, MUST_ESCALATE_IRI)?;
        let rationale = single_literal_property(store, iri, RATIONALE_IRI)?;
        if may_decide.is_empty() && must_escalate.is_empty() && rationale.is_none() {
            return Ok(None);
        }
        let escalate_via = load_escalation_hints(store, iri)?;
        Ok(Some(Self {
            iri: iri.to_string(),
            may_decide,
            must_escalate,
            escalate_via,
            rationale: rationale.unwrap_or_default(),
        }))
    }
}

fn load_escalation_hints(store: &Store, iri: &str) -> Result<Vec<EscalationHint>> {
    let q = build_escalation_hints_query(iri);
    let mut out: Vec<EscalationHint> = Vec::new();
    let QueryResults::Solutions(sols) = store.query(q.as_str()).context("escalateVia query")?
    else {
        return Ok(out);
    };
    for sol in sols {
        let sol = sol.context("escalateVia row")?;
        out.push(decode_escalation_hint(&sol)?);
    }
    Ok(out)
}

fn build_escalation_hints_query(iri: &str) -> String {
    format!(
        "PREFIX dec: <https://decision-cli.dev/ns#> \
         SELECT ?category ?class ?target WHERE {{ \
           {{ <{iri}> dec:escalateVia ?hint . \
              ?hint dec:escalateCategory ?category ; \
                    dec:escalateClass    ?class ; \
                    dec:escalateTargetRole ?target . }} \
           UNION \
           {{ GRAPH ?g {{ <{iri}> dec:escalateVia ?hint . \
              ?hint dec:escalateCategory ?category ; \
                    dec:escalateClass    ?class ; \
                    dec:escalateTargetRole ?target . }} }} \
         }} ORDER BY ?category",
    )
}

fn decode_escalation_hint(sol: &oxigraph::sparql::QuerySolution) -> Result<EscalationHint> {
    let category = bind_literal(sol, "category")?;
    let class_str = bind_literal(sol, "class")?;
    let target_role = bind_literal(sol, "target")?;
    let class = FeedbackClass::from_iri_value(&class_str).ok_or_else(|| {
        anyhow!(
            "escalateVia hint for category {category:?} has unknown class {class_str:?}; \
             must be one of the six ADR-023 values"
        )
    })?;
    Ok(EscalationHint {
        category,
        class,
        target_role,
    })
}

fn collect_literal_property(store: &Store, iri: &str, predicate: &str) -> Result<Vec<String>> {
    let q = format!(
        "SELECT ?v WHERE {{ \
           {{ <{iri}> <{predicate}> ?v . }} \
           UNION \
           {{ GRAPH ?g {{ <{iri}> <{predicate}> ?v . }} }} \
         }} ORDER BY ?v",
    );
    let mut out: Vec<String> = Vec::new();
    let QueryResults::Solutions(sols) = store
        .query(q.as_str())
        .with_context(|| format!("authority literal query for {predicate}"))?
    else {
        return Ok(out);
    };
    for sol in sols {
        let sol = sol.with_context(|| format!("authority literal row for {predicate}"))?;
        if let Some(oxigraph::model::Term::Literal(lit)) = sol.get("v") {
            out.push(lit.value().to_string());
        }
    }
    Ok(out)
}

fn single_literal_property(store: &Store, iri: &str, predicate: &str) -> Result<Option<String>> {
    let values = collect_literal_property(store, iri, predicate)?;
    Ok(values.into_iter().next())
}

fn bind_literal(row: &oxigraph::sparql::QuerySolution, name: &str) -> Result<String> {
    match row.get(name) {
        Some(oxigraph::model::Term::Literal(lit)) => Ok(lit.value().to_string()),
        Some(other) => Err(anyhow!("expected literal for ?{name}, got {other}")),
        None => Err(anyhow!("missing binding for ?{name}")),
    }
}
