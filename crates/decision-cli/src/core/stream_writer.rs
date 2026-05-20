//! Stream-aware mutation middleware over `oxi_events::GraphWriter`.
//!
//! decision-cli mutates the orchestration store exclusively through this
//! middleware (FT-010 / ADR-005). Every scoped artifact written here
//! gains a `dec:inStream` link to the active `dec:ValueStream`, making
//! TC-014's invariant structural rather than incidental.
//!
//! The middleware lives in the decision-cli crate (not in `oxi-events`)
//! because the `dec:` vocabulary is application-level — ADR-001 forbids
//! oxi-events from naming it.

use std::sync::Arc;

use anyhow::{anyhow, Context, Result};
use oxi_events::{CommitResult, GraphWriter, Mutation};
use oxigraph::model::{GraphName, NamedNode, NamedNodeRef, Quad, Term};
use oxigraph::store::Store;

use crate::core::feedback::validate_quads as validate_feedback_quads;
use crate::core::ontology::verdict::validate_quads as validate_verdict_quads;
use crate::core::vocab::{
    in_stream, orchestration_graph, value_stream_class, IRI_DEC_GRAPH_ORCHESTRATION,
    IRI_DEC_IN_STREAM, IRI_DEC_VALUE_STREAM, SCOPED_CLASSES,
};

const RDF_TYPE: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#type";

/// Mutation middleware that tags scoped artifacts with `dec:inStream`.
pub struct StreamWriter {
    inner: GraphWriter,
    active_stream: NamedNode,
}

impl StreamWriter {
    /// Bind a writer to the named stream. The caller is responsible for
    /// having already persisted the `ValueStream` artifact in the store
    /// (FT-008 / FT-009 own that bootstrap path).
    pub fn open(store: Arc<Store>, active_stream: NamedNode) -> Result<Self> {
        Self::ensure_stream_present(&store, &active_stream)?;
        let inner = GraphWriter::open(store).context("opening underlying graph writer")?;
        Ok(Self {
            inner,
            active_stream,
        })
    }

    /// Bootstrap helper: persist the `ValueStream` artifact and bind to it.
    ///
    /// FT-009 will replace this with a richer `OrchestrationStore::open`
    /// flow; for slice 1 the helper keeps tests and an early CLI path
    /// honest about the invariant.
    pub fn bootstrap(store: Arc<Store>, active_stream: NamedNode) -> Result<Self> {
        let graph: GraphName = orchestration_graph().into_owned().into();
        let rdf_type = NamedNodeRef::new(RDF_TYPE).context("rdf:type iri")?;
        let class = value_stream_class();
        let quad = Quad::new(active_stream.clone(), rdf_type, class.into_owned(), graph);
        store
            .transaction(|mut tx| tx.insert(quad.as_ref()).map(|_| ()))
            .context("seeding active value stream")?;
        Self::open(store, active_stream)
    }

    /// Active stream IRI.
    #[must_use]
    pub fn active_stream(&self) -> &NamedNode {
        &self.active_stream
    }

    /// Borrow the underlying writer (read-only inspection).
    #[must_use]
    pub fn inner(&self) -> &GraphWriter {
        &self.inner
    }

    /// Commit a mutation through the underlying writer, augmenting it
    /// with `dec:inStream` quads for any scoped artifact being declared.
    ///
    /// Verdict mutations (FT-020): every `dec:VerificationVerdict` subject
    /// in `mutation.inserts` is validated against the ADR-018 SHACL shape
    /// **after** the `dec:inStream` augmentation and **before** the
    /// underlying writer sees the mutation. A failing shape is converted
    /// to an `anyhow` error whose message starts with `SHACL violation`
    /// so callers can match on the prefix without depending on the
    /// internal error type.
    pub fn commit(&self, mutation: Mutation) -> Result<CommitResult> {
        let mutation = self.augment(mutation);
        validate_verdicts(&mutation.inserts)?;
        validate_feedback(&mutation.inserts)?;
        self.inner
            .commit(mutation)
            .context("committing mutation through oxi-events writer")
    }

    fn augment(&self, mut mutation: Mutation) -> Mutation {
        let in_stream_pred = in_stream().into_owned();
        let stream_obj: Term = self.active_stream.clone().into();
        let scoped: Vec<NamedNode> = scoped_subjects(&mutation);
        for subject in scoped {
            let graph = scoped_target_graph(&mutation, &subject);
            mutation.inserts.push(Quad::new(
                subject,
                in_stream_pred.clone(),
                stream_obj.clone(),
                graph,
            ));
        }
        mutation
    }

    fn ensure_stream_present(store: &Store, stream: &NamedNode) -> Result<()> {
        // The stream may live in the default graph (FT-008 persistence
        // path) or a named graph (FT-001 bootstrap path); accept both.
        let q = format!(
            "ASK {{ \
              {{ <{stream}> a <{vs}> }} \
              UNION \
              {{ GRAPH ?g {{ <{stream}> a <{vs}> }} }} \
            }}",
            stream = stream.as_str(),
            vs = IRI_DEC_VALUE_STREAM,
        );
        match store.query(q.as_str()).context("ask active stream")? {
            oxigraph::sparql::QueryResults::Boolean(true) => Ok(()),
            _ => Err(anyhow!(
                "active stream {} not present in orchestration store; seed it via bootstrap()",
                stream.as_str()
            )),
        }
    }
}

/// SHACL-validate every VerificationVerdict subject present in `quads`,
/// converting a failure into an `anyhow` error tagged with the
/// `SHACL violation` prefix so callers can detect verdict failures
/// without depending on the verdict module's error type.
fn validate_verdicts(quads: &[Quad]) -> Result<()> {
    validate_verdict_quads(quads).map_err(|err| {
        anyhow!(
            "SHACL violation: verification verdict mutation refused\n{}",
            err.report
        )
    })
}

/// SHACL-validate every `dec:Feedback` subject present in `quads`
/// (FT-026 / ADR-022). The error message keeps the `SHACL violation`
/// prefix used by [`validate_verdicts`] so existing callers that match
/// on the prefix continue to work uniformly.
fn validate_feedback(quads: &[Quad]) -> Result<()> {
    validate_feedback_quads(quads).map_err(|err| {
        anyhow!(
            "SHACL violation: feedback mutation refused\n{}",
            err.report
        )
    })
}

fn scoped_subjects(mutation: &Mutation) -> Vec<NamedNode> {
    let rdf_type = RDF_TYPE;
    let mut out: Vec<NamedNode> = Vec::new();
    for quad in &mutation.inserts {
        if quad.predicate.as_str() != rdf_type {
            continue;
        }
        let Term::NamedNode(cls) = &quad.object else {
            continue;
        };
        if !SCOPED_CLASSES.contains(&cls.as_str()) {
            continue;
        }
        let Some(subject) = named_subject(quad) else {
            continue;
        };
        if !out.iter().any(|s| s == &subject) {
            out.push(subject);
        }
    }
    out
}

fn named_subject(quad: &Quad) -> Option<NamedNode> {
    use oxigraph::model::Subject;
    match &quad.subject {
        Subject::NamedNode(n) => Some(n.clone()),
        _ => None,
    }
}

fn scoped_target_graph(mutation: &Mutation, subject: &NamedNode) -> GraphName {
    for q in &mutation.inserts {
        if let oxigraph::model::Subject::NamedNode(s) = &q.subject {
            if s == subject {
                return q.graph_name.clone();
            }
        }
    }
    GraphName::NamedNode(NamedNode::new_unchecked(IRI_DEC_GRAPH_ORCHESTRATION))
}

/// Convenience: build a `dec:inStream` quad for a subject living in `graph`.
#[must_use]
pub fn in_stream_quad(subject: NamedNode, stream: &NamedNode, graph: GraphName) -> Quad {
    Quad::new(
        subject,
        NamedNodeRef::new_unchecked(IRI_DEC_IN_STREAM).into_owned(),
        stream.clone(),
        graph,
    )
}
