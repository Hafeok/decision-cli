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
use oxigraph::model::{GraphName, NamedNode, NamedNodeRef, Quad, Subject, Term};
use oxigraph::store::Store;

use crate::core::feedback::lifecycle::{validate_transition, LifecycleState};
use crate::core::ontology::session_record::validate_quads_with_store as validate_session_record_quads_with_store;
use crate::core::stream_writer_validations::{
    validate_bundles, validate_capabilities, validate_envs, validate_feedback, validate_graphs,
    validate_results, validate_role_bindings, validate_verdicts, validate_waivers,
};
use crate::core::verify::quads::{check_inserts_against_store, touches_verification_artifacts};
use crate::core::vocab::{
    in_stream, lifecycle_state, orchestration_graph, value_stream_class, IRI_DEC_FEEDBACK,
    IRI_DEC_GRAPH_ORCHESTRATION, IRI_DEC_IN_STREAM, IRI_DEC_LIFECYCLE_STATE, IRI_DEC_VALUE_STREAM,
    SCOPED_CLASSES,
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
        // FT-037 / ADR-028 §Safety gating: safety check runs *before*
        // any SHACL pass and before the underlying writer sees the
        // mutation. SHACL and safety errors are distinct (TC-059 §5).
        self.validate_safety(&mutation.inserts)?;
        validate_verdicts(&mutation.inserts)?;
        validate_feedback(&mutation.inserts)?;
        validate_envs(&mutation.inserts)?;
        validate_graphs(&mutation.inserts)?;
        validate_results(&mutation.inserts, Some(self.inner.store().as_ref()))?;
        validate_waivers(&mutation.inserts)?;
        validate_capabilities(&mutation.inserts)?;
        validate_role_bindings(&mutation.inserts)?;
        validate_bundles(&mutation.inserts)?;
        self.validate_session_records(&mutation.inserts)?;
        self.validate_feedback_transitions(&mutation)?;
        self.inner
            .commit(mutation)
            .context("committing mutation through oxi-events writer")
    }

    /// FT-037 chokepoint — runs the safety check on every
    /// `VerificationGraph`/`Step` subject in `inserts`. No-op when the
    /// mutation does not touch a verification artifact. Surfaces
    /// `Error::SafetyViolation` (rendered with the `safety violation`
    /// prefix so callers can match on it without depending on the
    /// safety module's error type).
    fn validate_safety(&self, inserts: &[Quad]) -> Result<()> {
        if !touches_verification_artifacts(inserts) {
            return Ok(());
        }
        let store = self.inner.store();
        match check_inserts_against_store(inserts, store) {
            Ok(()) => Ok(()),
            Err(err) => Err(anyhow!("safety violation: {err}")),
        }
    }

    /// FT-057 / ADR-034 / ADR-037: SHACL-validate every `dec:Session`
    /// subject in `inserts`. The store is consulted so Scaleway-no-cache
    /// can resolve the session's capability endpoint even when the
    /// capability was seeded by a prior mutation (e.g. by the catalog
    /// bootstrap in FT-058).
    fn validate_session_records(&self, inserts: &[Quad]) -> Result<()> {
        let store = self.inner.store();
        validate_session_record_quads_with_store(inserts, Some(store)).map_err(|err| {
            anyhow!(
                "SHACL violation: session record mutation refused\n{}",
                err.report
            )
        })
    }

    /// FT-027 / ADR-024: when a mutation updates `dec:lifecycleState` on a
    /// `dec:Feedback` that already exists in the store, the prior state
    /// is read and the transition is validated against
    /// [`validate_transition`]. New feedback (no prior state) is exempt;
    /// `produced` is the seed.
    fn validate_feedback_transitions(&self, mutation: &Mutation) -> Result<()> {
        let store = self.inner.store();
        for (subject, raw_to) in lifecycle_updates(&mutation.inserts) {
            // Skip subjects that are being created in this same mutation —
            // an inserted `rdf:type dec:Feedback` triple means there is no
            // prior state to transition from.
            if subject_being_created(&mutation.inserts, &subject) {
                continue;
            }
            let Some(prior_raw) = read_stored_lifecycle(store, &subject) else {
                // No prior state and no new-creation declaration in this
                // mutation: SHACL has already caught the malformed write
                // (rdf:type missing); skip transition validation.
                continue;
            };
            validate_single_transition(&subject, &prior_raw, &raw_to)?;
        }
        Ok(())
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

/// Validate a single lifecycle transition for `subject` going from
/// `prior_raw` to `raw_to`. Self-loops (`from == to`) are dropped to
/// allow re-asserts; unknown labels surface as `SHACL violation` errors.
fn validate_single_transition(subject: &NamedNode, prior_raw: &str, raw_to: &str) -> Result<()> {
    let from = LifecycleState::parse(prior_raw).ok_or_else(|| {
        anyhow!(
            "SHACL violation: prior dec:lifecycleState for <{subj}> is malformed: {prior_raw:?}",
            subj = subject.as_str(),
        )
    })?;
    let to = LifecycleState::parse(raw_to).ok_or_else(|| {
        anyhow!(
            "SHACL violation: feedback mutation refused — unknown target lifecycle state {raw_to:?}",
        )
    })?;
    if from == to {
        // No-op transitions are dropped here: mutation re-asserts the
        // same state; we skip the validator on a self-loop.
        return Ok(());
    }
    validate_transition(from, to).map_err(|err| {
        anyhow!(
            "SHACL violation: feedback mutation refused — invalid lifecycle transition for <{subj}>: {err}",
            subj = subject.as_str(),
        )
    })
}

/// SHACL-validate every VerificationVerdict subject present in `quads`,
/// Walk a mutation's inserts and yield every `(subject, new_state)`
/// pair declared by a `dec:lifecycleState` literal triple. Only `Feedback`
/// subjects are returned (filtered by inspecting either the inserted
/// `rdf:type` triples or the prior store state in the caller).
fn lifecycle_updates(inserts: &[Quad]) -> Vec<(NamedNode, String)> {
    let pred = IRI_DEC_LIFECYCLE_STATE;
    let mut out: Vec<(NamedNode, String)> = Vec::new();
    for q in inserts {
        if q.predicate.as_str() != pred {
            continue;
        }
        let Subject::NamedNode(s) = &q.subject else {
            continue;
        };
        let Term::Literal(lit) = &q.object else {
            continue;
        };
        let pair = (s.clone(), lit.value().to_string());
        if !out.iter().any(|(p, _)| p == &pair.0) {
            out.push(pair);
        }
    }
    out
}

/// True if `subject` is being declared as a new `dec:Feedback` artifact
/// in the same mutation (an `rdf:type dec:Feedback` insert triple).
fn subject_being_created(inserts: &[Quad], subject: &NamedNode) -> bool {
    for q in inserts {
        if q.predicate.as_str() != RDF_TYPE {
            continue;
        }
        let Subject::NamedNode(s) = &q.subject else {
            continue;
        };
        if s != subject {
            continue;
        }
        if let Term::NamedNode(cls) = &q.object {
            if cls.as_str() == IRI_DEC_FEEDBACK {
                return true;
            }
        }
    }
    false
}

/// Read the current `dec:lifecycleState` literal for `subject` from any
/// graph in `store`. Returns `None` if no such literal exists yet.
fn read_stored_lifecycle(store: &Store, subject: &NamedNode) -> Option<String> {
    let pred = lifecycle_state();
    for quad in store
        .quads_for_pattern(
            Some(Subject::NamedNode(subject.clone()).as_ref()),
            Some(pred),
            None,
            None,
        )
        .filter_map(Result::ok)
    {
        if let Term::Literal(lit) = quad.object {
            return Some(lit.value().to_string());
        }
    }
    None
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
