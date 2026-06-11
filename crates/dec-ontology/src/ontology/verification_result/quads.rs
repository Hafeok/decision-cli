//! RDF quad serialisation for `VerificationGraphResult` /
//! `VerificationStepTrace` (FT-097 / ADR-028).
//!
//! Pure write-side helpers. Reading lives in [`super::io`].

use oxrdf::{BlankNode, GraphName, Literal, NamedNode, NamedNodeRef, Quad, Subject, Term};

use crate::vocab::{
    ended_at_pred, error_message_pred, evidence_for_pred, evidence_projection_class,
    exit_code_pred, outcome_pred, ran_on_bench_pred, rationale, result_of_pred, started_at_pred,
    stderr_excerpt_pred, stdout_excerpt_pred, step_traces_pred, tc_pred, traces_step_pred, verdict,
    verification_graph_result_class, verification_step_trace_class,
};

use super::types::{
    VerificationGraphResult, VerificationStepTrace, DCTERMS_CREATED, PROV_WAS_ATTRIBUTED_TO,
    PROV_WAS_GENERATED_BY, RDF_FIRST, RDF_NIL, RDF_REST, RDF_TYPE,
};

impl VerificationStepTrace {
    /// Body quads for the trace (everything except its rdf:List membership).
    #[must_use]
    pub fn to_quads(&self, graph: NamedNodeRef<'_>) -> Vec<Quad> {
        let g: GraphName = graph.into_owned().into();
        let id = NamedNode::new_unchecked(&self.id);
        let rdf_type = NamedNodeRef::new_unchecked(RDF_TYPE);
        let mut out = vec![
            Quad::new(
                id.clone(),
                rdf_type,
                verification_step_trace_class(),
                g.clone(),
            ),
            iri_quad(&id, traces_step_pred(), &self.traces_step, &g),
            literal_quad(&id, outcome_pred(), self.outcome.as_str(), &g),
            datetime_quad(&id, started_at_pred(), &self.started_at, &g),
            datetime_quad(&id, ended_at_pred(), &self.ended_at, &g),
            literal_quad(&id, stdout_excerpt_pred(), &self.stdout_excerpt, &g),
            literal_quad(&id, stderr_excerpt_pred(), &self.stderr_excerpt, &g),
            iri_quad(
                &id,
                NamedNodeRef::new_unchecked(PROV_WAS_GENERATED_BY),
                &self.was_generated_by,
                &g,
            ),
        ];
        if let Some(code) = self.exit_code {
            out.push(integer_quad(&id, exit_code_pred(), code, &g));
        }
        // errorMessage is required-as-string but only meaningful when
        // outcome != pass — we emit it unconditionally if non-empty.
        if !self.error_message.is_empty() {
            out.push(literal_quad(
                &id,
                error_message_pred(),
                &self.error_message,
                &g,
            ));
        }
        out
    }
}

impl VerificationGraphResult {
    /// Serialise the result (and every step trace + evidence projection) to
    /// RDF quads in `graph`.
    #[must_use]
    pub fn to_quads(&self, graph: NamedNodeRef<'_>) -> Vec<Quad> {
        let g: GraphName = graph.into_owned().into();
        let id = NamedNode::new_unchecked(&self.id);
        let rdf_type = NamedNodeRef::new_unchecked(RDF_TYPE);
        let mut quads = vec![
            Quad::new(
                id.clone(),
                rdf_type,
                verification_graph_result_class(),
                g.clone(),
            ),
            iri_quad(&id, result_of_pred(), &self.result_of, &g),
            iri_quad(&id, ran_on_bench_pred(), &self.ran_in_environment, &g),
            literal_quad(&id, verdict(), self.verdict.as_str(), &g),
            datetime_quad(&id, started_at_pred(), &self.started_at, &g),
            datetime_quad(&id, ended_at_pred(), &self.ended_at, &g),
            literal_quad(&id, rationale(), &self.rationale, &g),
            iri_quad(
                &id,
                NamedNodeRef::new_unchecked(PROV_WAS_GENERATED_BY),
                &self.was_generated_by,
                &g,
            ),
            iri_quad(
                &id,
                NamedNodeRef::new_unchecked(PROV_WAS_ATTRIBUTED_TO),
                &self.was_attributed_to,
                &g,
            ),
            datetime_quad(
                &id,
                NamedNodeRef::new_unchecked(DCTERMS_CREATED),
                &self.created_at,
                &g,
            ),
        ];
        quads.extend(step_traces_list_quads(self, &g));
        for trace in &self.step_traces {
            quads.extend(trace.to_quads(graph));
        }
        quads.extend(evidence_for_quads(self, &g));
        quads
    }
}

fn step_traces_list_quads(result: &VerificationGraphResult, g: &GraphName) -> Vec<Quad> {
    let parent = NamedNode::new_unchecked(&result.id);
    let steps_p = step_traces_pred();
    if result.step_traces.is_empty() {
        let rdf_nil = NamedNodeRef::new_unchecked(RDF_NIL);
        return vec![Quad::new(parent, steps_p.into_owned(), rdf_nil, g.clone())];
    }
    let id_str = result.id.as_str();
    let nodes: Vec<BlankNode> = (0..result.step_traces.len())
        .map(|i| BlankNode::new_unchecked(format!("traces-{}-{i}", suffix_for_id(id_str))))
        .collect();
    let mut quads = vec![Quad::new(
        parent,
        steps_p.into_owned(),
        nodes[0].clone(),
        g.clone(),
    )];
    let rdf_first = NamedNodeRef::new_unchecked(RDF_FIRST);
    let rdf_rest = NamedNodeRef::new_unchecked(RDF_REST);
    let rdf_nil = NamedNodeRef::new_unchecked(RDF_NIL);
    for (i, trace) in result.step_traces.iter().enumerate() {
        let head = Subject::BlankNode(nodes[i].clone());
        let trace_iri = NamedNode::new_unchecked(&trace.id);
        quads.push(Quad::new(
            head.clone(),
            rdf_first.into_owned(),
            trace_iri,
            g.clone(),
        ));
        let rest_term: Term = if i + 1 < nodes.len() {
            Term::BlankNode(nodes[i + 1].clone())
        } else {
            rdf_nil.into_owned().into()
        };
        quads.push(Quad::new(head, rdf_rest.into_owned(), rest_term, g.clone()));
    }
    quads
}

fn evidence_for_quads(result: &VerificationGraphResult, g: &GraphName) -> Vec<Quad> {
    let mut out = Vec::new();
    let parent = NamedNode::new_unchecked(&result.id);
    let rdf_type = NamedNodeRef::new_unchecked(RDF_TYPE);
    let id_suffix = suffix_for_id(result.id.as_str());
    for (i, projection) in result.evidence_for.iter().enumerate() {
        let bn = BlankNode::new_unchecked(format!("ev-{}-{i}", id_suffix));
        out.push(Quad::new(
            parent.clone(),
            evidence_for_pred().into_owned(),
            bn.clone(),
            g.clone(),
        ));
        out.push(Quad::new(
            Subject::BlankNode(bn.clone()),
            rdf_type.into_owned(),
            evidence_projection_class(),
            g.clone(),
        ));
        out.push(Quad::new(
            Subject::BlankNode(bn.clone()),
            tc_pred().into_owned(),
            NamedNode::new_unchecked(&projection.tc),
            g.clone(),
        ));
        out.push(Quad::new(
            Subject::BlankNode(bn.clone()),
            outcome_pred().into_owned(),
            Literal::new_simple_literal(projection.outcome.as_str()),
            g.clone(),
        ));
        out.push(Quad::new(
            Subject::BlankNode(bn),
            NamedNode::new_unchecked("https://decision-cli.dev/ns#fromStep"),
            NamedNode::new_unchecked(&projection.from_step),
            g.clone(),
        ));
    }
    out
}

fn suffix_for_id(iri: &str) -> String {
    // Last path segment; falls back to a hash-friendly safe string.
    iri.rsplit('/').next().unwrap_or("vgr").to_string()
}

pub(super) fn literal_quad(s: &NamedNode, p: NamedNodeRef<'_>, value: &str, g: &GraphName) -> Quad {
    Quad::new(
        s.clone(),
        p.into_owned(),
        Literal::new_simple_literal(value),
        g.clone(),
    )
}

pub(super) fn iri_quad(s: &NamedNode, p: NamedNodeRef<'_>, value: &str, g: &GraphName) -> Quad {
    Quad::new(
        s.clone(),
        p.into_owned(),
        NamedNode::new_unchecked(value),
        g.clone(),
    )
}

fn datetime_quad(s: &NamedNode, p: NamedNodeRef<'_>, value: &str, g: &GraphName) -> Quad {
    let lit = Literal::new_typed_literal(
        value,
        NamedNode::new_unchecked("http://www.w3.org/2001/XMLSchema#dateTime"),
    );
    Quad::new(s.clone(), p.into_owned(), lit, g.clone())
}

fn integer_quad(s: &NamedNode, p: NamedNodeRef<'_>, value: i64, g: &GraphName) -> Quad {
    let lit = Literal::new_typed_literal(
        value.to_string(),
        NamedNode::new_unchecked("http://www.w3.org/2001/XMLSchema#integer"),
    );
    Quad::new(s.clone(), p.into_owned(), lit, g.clone())
}
