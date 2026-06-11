//! In-memory shapes + RDF serialisation for the three catalog types.
//!
//! Per ADR-066 §Rule 2 the bundle assembler reads catalog artifacts via
//! SPARQL `CONSTRUCT`; the on-disk and in-store form is plain Turtle.
//! These types are the Rust mirror of that shape: each carries the
//! fields the SHACL validator (sibling [`super::shacl`]) checks.

#![allow(missing_docs)]

use oxrdf::{GraphName, Literal, NamedNode, NamedNodeRef, Quad, Term};

use crate::vocab::{
    applies_to_safety_class, based_on_approved_result, capability_body, capability_reference_class,
    capability_version_cr, catalog_graph, command_cr, exemplar_graph_class, exemplar_of,
    namespace_pred, ontology_body, ontology_description_class, ontology_version_pred, pattern_name,
    prefix_pred, IRI_DEC_CR_PREFIX, IRI_DEC_EX_PREFIX, IRI_DEC_OD_PREFIX, SAFETY_ISOLATED,
    SAFETY_PRODUCTION_READONLY, SAFETY_SHARED_NON_DESTRUCTIVE,
};

/// IRI of `rdf:type`.
pub const RDF_TYPE: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#type";
/// IRI of `dec:supersedes`.
pub const SUPERSEDES: &str = "https://decision-cli.dev/ns#supersedes";
/// IRI of `dec:supersededBy`.
pub const SUPERSEDED_BY: &str = "https://decision-cli.dev/ns#supersededBy";

/// Safety-class tag accepted by [`ExemplarGraph`] (ADR-028's controlled
/// vocabulary; duplicated here to keep the catalog types decoupled from
/// the verification_bench enum's `Display` impl).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SafetyClassTag {
    Isolated,
    SharedNonDestructive,
    ProductionReadonly,
}

impl SafetyClassTag {
    /// Stable wire string.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Isolated => SAFETY_ISOLATED,
            Self::SharedNonDestructive => SAFETY_SHARED_NON_DESTRUCTIVE,
            Self::ProductionReadonly => SAFETY_PRODUCTION_READONLY,
        }
    }

    /// Parse from wire string; unknown values return `None`.
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            SAFETY_ISOLATED => Some(Self::Isolated),
            SAFETY_SHARED_NON_DESTRUCTIVE => Some(Self::SharedNonDestructive),
            SAFETY_PRODUCTION_READONLY => Some(Self::ProductionReadonly),
            _ => None,
        }
    }
}

/// In-memory `dec:CapabilityReference` artifact.
///
/// Identity is the `id` field (e.g. `CR-001`); the canonical IRI is
/// `https://decision-cli.dev/ns/cr/<id>`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityReference {
    pub id: String,
    pub command: String,
    pub capability_version: String,
    /// JSON literal carrying the structured body. Validated against the
    /// Pydantic-mirrored schema upstream; persisted as opaque text here.
    pub body: String,
    /// Optional `dec:supersedes` link to a prior CR for the same command.
    pub supersedes: Option<String>,
}

impl CapabilityReference {
    /// Canonical IRI for this reference.
    #[must_use]
    pub fn iri(&self) -> NamedNode {
        NamedNode::new_unchecked(format!("{IRI_DEC_CR_PREFIX}{id}", id = self.id))
    }

    /// Serialise to quads in the catalog named graph.
    #[must_use]
    pub fn to_quads(&self) -> Vec<Quad> {
        let g: GraphName = catalog_graph().into_owned().into();
        let subject = self.iri();
        let mut quads = vec![
            type_quad(&subject, capability_reference_class(), &g),
            literal_quad(&subject, command_cr(), &self.command, &g),
            literal_quad(
                &subject,
                capability_version_cr(),
                &self.capability_version,
                &g,
            ),
            literal_quad(&subject, capability_body(), &self.body, &g),
        ];
        if let Some(prior_id) = &self.supersedes {
            let target = NamedNode::new_unchecked(format!("{IRI_DEC_CR_PREFIX}{prior_id}"));
            quads.push(Quad::new(
                subject.clone(),
                NamedNodeRef::new_unchecked(SUPERSEDES).into_owned(),
                target.clone(),
                g.clone(),
            ));
            // Inverse edge on the old artifact so list --active SPARQL
            // walks honour the supersession.
            quads.push(Quad::new(
                target,
                NamedNodeRef::new_unchecked(SUPERSEDED_BY).into_owned(),
                Term::NamedNode(subject.clone()),
                g,
            ));
        }
        quads
    }
}

/// In-memory `dec:OntologyDescription` artifact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OntologyDescription {
    pub id: String,
    pub namespace: String,
    pub prefix: String,
    pub ontology_version: String,
    pub body: String,
    pub supersedes: Option<String>,
}

impl OntologyDescription {
    #[must_use]
    pub fn iri(&self) -> NamedNode {
        NamedNode::new_unchecked(format!("{IRI_DEC_OD_PREFIX}{id}", id = self.id))
    }

    #[must_use]
    pub fn to_quads(&self) -> Vec<Quad> {
        let g: GraphName = catalog_graph().into_owned().into();
        let subject = self.iri();
        let mut quads = vec![
            type_quad(&subject, ontology_description_class(), &g),
            literal_quad(&subject, namespace_pred(), &self.namespace, &g),
            literal_quad(&subject, prefix_pred(), &self.prefix, &g),
            literal_quad(
                &subject,
                ontology_version_pred(),
                &self.ontology_version,
                &g,
            ),
            literal_quad(&subject, ontology_body(), &self.body, &g),
        ];
        if let Some(prior_id) = &self.supersedes {
            let target = NamedNode::new_unchecked(format!("{IRI_DEC_OD_PREFIX}{prior_id}"));
            quads.push(Quad::new(
                subject.clone(),
                NamedNodeRef::new_unchecked(SUPERSEDES).into_owned(),
                target.clone(),
                g.clone(),
            ));
            quads.push(Quad::new(
                target,
                NamedNodeRef::new_unchecked(SUPERSEDED_BY).into_owned(),
                Term::NamedNode(subject.clone()),
                g,
            ));
        }
        quads
    }
}

/// In-memory `dec:ExemplarGraph` artifact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExemplarGraph {
    pub id: String,
    /// IRI of the underlying `dec:VerificationGraph` being promoted.
    pub exemplar_of: NamedNode,
    pub applies_to_safety_class: SafetyClassTag,
    pub pattern_name: String,
    pub rationale: String,
    /// IRI of the backing `dec:VerificationGraphResult` whose verdict =
    /// approved AND resultOf = `exemplar_of`. SHACL refuses promotion
    /// without it (TC-167).
    pub based_on_approved_result: NamedNode,
    pub supersedes: Option<String>,
}

impl ExemplarGraph {
    #[must_use]
    pub fn iri(&self) -> NamedNode {
        NamedNode::new_unchecked(format!("{IRI_DEC_EX_PREFIX}{id}", id = self.id))
    }

    #[must_use]
    pub fn to_quads(&self) -> Vec<Quad> {
        use crate::vocab::rationale;
        let g: GraphName = catalog_graph().into_owned().into();
        let subject = self.iri();
        let mut quads = vec![
            type_quad(&subject, exemplar_graph_class(), &g),
            Quad::new(
                subject.clone(),
                exemplar_of().into_owned(),
                Term::NamedNode(self.exemplar_of.clone()),
                g.clone(),
            ),
            literal_quad(
                &subject,
                applies_to_safety_class(),
                self.applies_to_safety_class.as_str(),
                &g,
            ),
            literal_quad(&subject, pattern_name(), &self.pattern_name, &g),
            literal_quad(&subject, rationale(), &self.rationale, &g),
            Quad::new(
                subject.clone(),
                based_on_approved_result().into_owned(),
                Term::NamedNode(self.based_on_approved_result.clone()),
                g.clone(),
            ),
        ];
        if let Some(prior_id) = &self.supersedes {
            let target = NamedNode::new_unchecked(format!("{IRI_DEC_EX_PREFIX}{prior_id}"));
            quads.push(Quad::new(
                subject.clone(),
                NamedNodeRef::new_unchecked(SUPERSEDES).into_owned(),
                target.clone(),
                g.clone(),
            ));
            quads.push(Quad::new(
                target,
                NamedNodeRef::new_unchecked(SUPERSEDED_BY).into_owned(),
                Term::NamedNode(subject.clone()),
                g,
            ));
        }
        quads
    }
}

fn type_quad(subject: &NamedNode, cls: NamedNodeRef<'_>, g: &GraphName) -> Quad {
    Quad::new(
        subject.clone(),
        NamedNodeRef::new_unchecked(RDF_TYPE).into_owned(),
        cls.into_owned(),
        g.clone(),
    )
}

fn literal_quad(s: &NamedNode, p: NamedNodeRef<'_>, value: &str, g: &GraphName) -> Quad {
    Quad::new(
        s.clone(),
        p.into_owned(),
        Literal::new_simple_literal(value),
        g.clone(),
    )
}
