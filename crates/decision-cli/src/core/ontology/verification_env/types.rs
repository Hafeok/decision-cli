//! In-memory `VerificationEnvironment` shape + RDF serialisation (ADR-028).

use oxigraph::model::{
    BlankNode, GraphName, Literal, NamedNode, NamedNodeRef, Quad, Subject, Term,
};

use crate::core::vocab::{
    allowed_ops, endpoint_pred, env_type, fixture_source_pred, safety_class, setup_pred,
    teardown_pred, verification_environment_class, IRI_DEC_ENV_PREFIX, SAFETY_ISOLATED,
    SAFETY_PRODUCTION_READONLY, SAFETY_SHARED_NON_DESTRUCTIVE,
};

pub(super) const RDF_TYPE: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#type";
pub(super) const RDF_FIRST: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#first";
pub(super) const RDF_REST: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#rest";
pub(super) const RDF_NIL: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#nil";

/// Safety classification per ADR-028 §VerificationEnvironment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SafetyClass {
    /// Sandboxed; failure does not affect other systems.
    Isolated,
    /// Multi-tenant environment; reads and non-mutating writes allowed.
    SharedNonDestructive,
    /// Production; only read operations permitted.
    ProductionReadonly,
}

impl SafetyClass {
    /// Stable wire string for the safety class.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Isolated => SAFETY_ISOLATED,
            Self::SharedNonDestructive => SAFETY_SHARED_NON_DESTRUCTIVE,
            Self::ProductionReadonly => SAFETY_PRODUCTION_READONLY,
        }
    }

    /// Parse a safety class from its wire string. Returns `None` for
    /// unknown values; the SHACL `sh:in` constraint catches them at
    /// commit time.
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

/// In-memory `dec:VerificationEnvironment` artifact.
///
/// Identity is the `id` field (e.g. `ENV-001-ephemeral-cli`); the
/// canonical IRI is `https://decision-cli.dev/ns/env/<id>`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerificationEnvironment {
    /// Identifier — e.g. `ENV-001-ephemeral-cli`.
    pub id: String,
    /// Environment type tag (e.g. `ephemeral-tempdir`, `remote-http`).
    pub env_type: String,
    /// Optional shell snippet run before any step.
    pub setup: Option<String>,
    /// Optional shell snippet run after step execution.
    pub teardown: Option<String>,
    /// Operation tokens permitted (rdf:List on disk, ordered here).
    pub allowed_ops: Vec<String>,
    /// Safety classification.
    pub safety_class: SafetyClass,
    /// Required for remote env types; forbidden for local types.
    pub endpoint: Option<String>,
    /// Repo-relative path to a fixture tree materialised before steps
    /// execute (FT-053 / ADR-032). `None` when the env carries no fixture.
    pub fixture_source: Option<String>,
}

impl VerificationEnvironment {
    /// Construct the canonical IRI for this environment.
    #[must_use]
    pub fn iri(&self) -> NamedNode {
        NamedNode::new_unchecked(format!("{IRI_DEC_ENV_PREFIX}{id}", id = self.id))
    }

    /// True iff this env's `env_type` is "remote-*" (ADR-028 §SHACL conditional).
    #[must_use]
    pub fn is_remote(&self) -> bool {
        self.env_type.starts_with("remote-")
    }

    /// Serialise the env to RDF quads in the supplied named graph.
    ///
    /// `allowed_ops` is serialised as an rdf:List of plain-string
    /// literals; an empty list collapses to `dec:allowedOps rdf:nil`,
    /// which the SHACL shape refuses.
    #[must_use]
    pub fn to_quads(&self, graph: NamedNodeRef<'_>) -> Vec<Quad> {
        let g: GraphName = graph.into_owned().into();
        let subject = self.iri();
        let mut quads = self.header_quads(&subject, &g);
        quads.extend(self.optional_quads(&subject, &g));
        quads.extend(self.allowed_ops_quads(&subject, &g));
        quads
    }

    fn header_quads(&self, subject: &NamedNode, g: &GraphName) -> Vec<Quad> {
        let rdf_type = NamedNodeRef::new_unchecked(RDF_TYPE);
        let cls = verification_environment_class();
        vec![
            Quad::new(subject.clone(), rdf_type, cls, g.clone()),
            literal_quad(subject, env_type(), &self.env_type, g),
            literal_quad(subject, safety_class(), self.safety_class.as_str(), g),
        ]
    }

    fn optional_quads(&self, subject: &NamedNode, g: &GraphName) -> Vec<Quad> {
        let mut quads = Vec::new();
        if let Some(s) = &self.setup {
            quads.push(literal_quad(subject, setup_pred(), s, g));
        }
        if let Some(s) = &self.teardown {
            quads.push(literal_quad(subject, teardown_pred(), s, g));
        }
        if let Some(e) = &self.endpoint {
            quads.push(literal_quad(subject, endpoint_pred(), e, g));
        }
        if let Some(f) = &self.fixture_source {
            quads.push(literal_quad(subject, fixture_source_pred(), f, g));
        }
        quads
    }

    fn allowed_ops_quads(&self, subject: &NamedNode, g: &GraphName) -> Vec<Quad> {
        if self.allowed_ops.is_empty() {
            let rdf_nil = NamedNodeRef::new_unchecked(RDF_NIL);
            return vec![Quad::new(
                subject.clone(),
                allowed_ops().into_owned(),
                rdf_nil,
                g.clone(),
            )];
        }
        let nodes: Vec<BlankNode> = (0..self.allowed_ops.len())
            .map(|i| BlankNode::new_unchecked(format!("ops-{id}-{i}", id = self.id)))
            .collect();
        let mut quads = vec![Quad::new(
            subject.clone(),
            allowed_ops().into_owned(),
            nodes[0].clone(),
            g.clone(),
        )];
        quads.extend(allowed_ops_list_quads(&nodes, &self.allowed_ops, g));
        quads
    }
}

fn allowed_ops_list_quads(nodes: &[BlankNode], ops: &[String], g: &GraphName) -> Vec<Quad> {
    let rdf_first = NamedNodeRef::new_unchecked(RDF_FIRST);
    let rdf_rest = NamedNodeRef::new_unchecked(RDF_REST);
    let rdf_nil = NamedNodeRef::new_unchecked(RDF_NIL);
    let mut out = Vec::with_capacity(ops.len() * 2);
    for (i, op) in ops.iter().enumerate() {
        let head = Subject::BlankNode(nodes[i].clone());
        out.push(Quad::new(
            head.clone(),
            rdf_first.into_owned(),
            Literal::new_simple_literal(op),
            g.clone(),
        ));
        let rest_term: Term = if i + 1 < nodes.len() {
            Term::BlankNode(nodes[i + 1].clone())
        } else {
            rdf_nil.into_owned().into()
        };
        out.push(Quad::new(head, rdf_rest.into_owned(), rest_term, g.clone()));
    }
    out
}

pub(super) fn literal_quad(s: &NamedNode, p: NamedNodeRef<'_>, value: &str, g: &GraphName) -> Quad {
    Quad::new(
        s.clone(),
        p.into_owned(),
        Literal::new_simple_literal(value),
        g.clone(),
    )
}
