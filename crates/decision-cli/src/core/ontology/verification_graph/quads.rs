//! RDF quad serialisation for `VerificationGraph` / `VerificationStep`
//! (ADR-028). Pure write-side helpers; reading lives in [`super::io`].

use oxigraph::model::{
    BlankNode, GraphName, Literal, NamedNode, NamedNodeRef, Quad, Subject, Term,
};

use crate::core::vocab::{
    environment_pred, provides_evidence_for_pred, step_type_pred, steps_pred,
    verification_graph_class, verification_step_class, verifies_pred, IRI_DEC_BIND_AS,
    IRI_DEC_CAPTURE_OUTPUT, IRI_DEC_COMMAND, IRI_DEC_CONDITION, IRI_DEC_EXPECT_CONTENT,
    IRI_DEC_EXPECT_EXIT_CODE, IRI_DEC_EXPECT_HASH, IRI_DEC_EXPECT_ROWS, IRI_DEC_EXPECT_STATUS,
    IRI_DEC_FROM_STEP, IRI_DEC_METHOD, IRI_DEC_PATH, IRI_DEC_QUERY, IRI_DEC_TARGET,
    IRI_DEC_TIMEOUT, IRI_DEC_URL,
};

use super::types::{
    StepFields, VerificationGraph, VerificationStep, RDF_FIRST, RDF_NIL, RDF_REST, RDF_TYPE,
};

impl VerificationStep {
    /// Body quads for the step (everything except its rdf:List membership).
    #[must_use]
    pub fn to_quads(&self, graph: NamedNodeRef<'_>) -> Vec<Quad> {
        let g: GraphName = graph.into_owned().into();
        let mut quads = Vec::new();
        let rdf_type = NamedNodeRef::new_unchecked(RDF_TYPE);
        quads.push(Quad::new(
            self.id.clone(),
            rdf_type,
            verification_step_class(),
            g.clone(),
        ));
        quads.push(literal_quad(
            &self.id,
            step_type_pred(),
            self.kind.as_str(),
            &g,
        ));
        for tc in &self.provides_evidence_for {
            quads.push(Quad::new(
                self.id.clone(),
                provides_evidence_for_pred().into_owned(),
                tc.clone(),
                g.clone(),
            ));
        }
        quads.extend(field_quads(&self.id, &self.fields, &g));
        quads
    }
}

impl VerificationGraph {
    /// Serialise the graph (and every step) to RDF quads in `graph`.
    #[must_use]
    pub fn to_quads(&self, graph: NamedNodeRef<'_>) -> Vec<Quad> {
        let g: GraphName = graph.into_owned().into();
        let mut quads = Vec::new();
        let rdf_type = NamedNodeRef::new_unchecked(RDF_TYPE);
        quads.push(Quad::new(
            self.id.clone(),
            rdf_type,
            verification_graph_class(),
            g.clone(),
        ));
        quads.push(Quad::new(
            self.id.clone(),
            verifies_pred().into_owned(),
            self.verifies.0.clone(),
            g.clone(),
        ));
        quads.push(Quad::new(
            self.id.clone(),
            environment_pred().into_owned(),
            self.environment.clone(),
            g.clone(),
        ));
        quads.extend(steps_list_quads(self, &g));
        for step in &self.steps {
            quads.extend(step.to_quads(graph));
        }
        quads
    }
}

fn steps_list_quads(graph: &VerificationGraph, g: &GraphName) -> Vec<Quad> {
    let rdf_first = NamedNodeRef::new_unchecked(RDF_FIRST);
    let rdf_rest = NamedNodeRef::new_unchecked(RDF_REST);
    let rdf_nil = NamedNodeRef::new_unchecked(RDF_NIL);
    let steps_p = steps_pred();
    if graph.steps.is_empty() {
        return vec![Quad::new(
            graph.id.clone(),
            steps_p.into_owned(),
            rdf_nil,
            g.clone(),
        )];
    }
    let id_str = graph.id_str();
    let nodes: Vec<BlankNode> = (0..graph.steps.len())
        .map(|i| BlankNode::new_unchecked(format!("steps-{id_str}-{i}")))
        .collect();
    let mut quads = vec![Quad::new(
        graph.id.clone(),
        steps_p.into_owned(),
        nodes[0].clone(),
        g.clone(),
    )];
    for (i, step) in graph.steps.iter().enumerate() {
        let head = Subject::BlankNode(nodes[i].clone());
        quads.push(Quad::new(
            head.clone(),
            rdf_first.into_owned(),
            step.id.clone(),
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

fn field_quads(s: &NamedNode, fields: &StepFields, g: &GraphName) -> Vec<Quad> {
    match fields {
        StepFields::ShellCommand {
            command,
            expect_exit_code,
            capture_output,
        } => shell_command_quads(s, command, *expect_exit_code, *capture_output, g),
        StepFields::SparqlAssertion {
            target,
            query,
            expect_rows,
        } => sparql_assertion_quads(s, target, query, *expect_rows, g),
        StepFields::FileAssertion {
            path,
            expect_hash,
            expect_content,
        } => file_assertion_quads(s, path, expect_hash.as_deref(), expect_content.as_deref(), g),
        StepFields::HttpRequest {
            method,
            url,
            expect_status,
        } => http_request_quads(s, method, url, *expect_status, g),
        StepFields::WaitFor { condition, timeout } => wait_for_quads(s, condition, timeout, g),
        StepFields::Capture { from_step, bind_as } => {
            capture_quads(s, from_step.as_ref(), bind_as, g)
        }
    }
}

fn shell_command_quads(
    s: &NamedNode,
    command: &str,
    expect_exit_code: Option<i64>,
    capture_output: Option<bool>,
    g: &GraphName,
) -> Vec<Quad> {
    let mut out = vec![literal_quad(
        s,
        NamedNodeRef::new_unchecked(IRI_DEC_COMMAND),
        command,
        g,
    )];
    if let Some(code) = expect_exit_code {
        out.push(integer_quad(s, IRI_DEC_EXPECT_EXIT_CODE, code, g));
    }
    if let Some(b) = capture_output {
        out.push(boolean_quad(s, IRI_DEC_CAPTURE_OUTPUT, b, g));
    }
    out
}

fn sparql_assertion_quads(
    s: &NamedNode,
    target: &str,
    query: &str,
    expect_rows: Option<i64>,
    g: &GraphName,
) -> Vec<Quad> {
    let mut out = vec![
        literal_quad(s, NamedNodeRef::new_unchecked(IRI_DEC_TARGET), target, g),
        literal_quad(s, NamedNodeRef::new_unchecked(IRI_DEC_QUERY), query, g),
    ];
    if let Some(rows) = expect_rows {
        out.push(integer_quad(s, IRI_DEC_EXPECT_ROWS, rows, g));
    }
    out
}

fn file_assertion_quads(
    s: &NamedNode,
    path: &str,
    expect_hash: Option<&str>,
    expect_content: Option<&str>,
    g: &GraphName,
) -> Vec<Quad> {
    let mut out = vec![literal_quad(
        s,
        NamedNodeRef::new_unchecked(IRI_DEC_PATH),
        path,
        g,
    )];
    if let Some(h) = expect_hash {
        out.push(literal_quad(
            s,
            NamedNodeRef::new_unchecked(IRI_DEC_EXPECT_HASH),
            h,
            g,
        ));
    }
    if let Some(c) = expect_content {
        out.push(literal_quad(
            s,
            NamedNodeRef::new_unchecked(IRI_DEC_EXPECT_CONTENT),
            c,
            g,
        ));
    }
    out
}

fn http_request_quads(
    s: &NamedNode,
    method: &str,
    url: &str,
    expect_status: Option<i64>,
    g: &GraphName,
) -> Vec<Quad> {
    let mut out = vec![
        literal_quad(s, NamedNodeRef::new_unchecked(IRI_DEC_METHOD), method, g),
        literal_quad(s, NamedNodeRef::new_unchecked(IRI_DEC_URL), url, g),
    ];
    if let Some(status) = expect_status {
        out.push(integer_quad(s, IRI_DEC_EXPECT_STATUS, status, g));
    }
    out
}

fn wait_for_quads(
    s: &NamedNode,
    condition: &NamedNode,
    timeout: &str,
    g: &GraphName,
) -> Vec<Quad> {
    vec![
        Quad::new(
            s.clone(),
            NamedNodeRef::new_unchecked(IRI_DEC_CONDITION).into_owned(),
            condition.clone(),
            g.clone(),
        ),
        literal_quad(s, NamedNodeRef::new_unchecked(IRI_DEC_TIMEOUT), timeout, g),
    ]
}

fn capture_quads(
    s: &NamedNode,
    from_step: Option<&NamedNode>,
    bind_as: &str,
    g: &GraphName,
) -> Vec<Quad> {
    let mut out = vec![literal_quad(
        s,
        NamedNodeRef::new_unchecked(IRI_DEC_BIND_AS),
        bind_as,
        g,
    )];
    if let Some(node) = from_step {
        out.push(Quad::new(
            s.clone(),
            NamedNodeRef::new_unchecked(IRI_DEC_FROM_STEP).into_owned(),
            node.clone(),
            g.clone(),
        ));
    }
    out
}

pub(super) fn literal_quad(
    s: &NamedNode,
    p: NamedNodeRef<'_>,
    value: &str,
    g: &GraphName,
) -> Quad {
    Quad::new(
        s.clone(),
        p.into_owned(),
        Literal::new_simple_literal(value),
        g.clone(),
    )
}

fn integer_quad(s: &NamedNode, predicate: &str, value: i64, g: &GraphName) -> Quad {
    let lit = Literal::new_typed_literal(
        value.to_string(),
        NamedNode::new_unchecked("http://www.w3.org/2001/XMLSchema#integer"),
    );
    Quad::new(
        s.clone(),
        NamedNode::new_unchecked(predicate),
        lit,
        g.clone(),
    )
}

fn boolean_quad(s: &NamedNode, predicate: &str, value: bool, g: &GraphName) -> Quad {
    let lit = Literal::new_typed_literal(
        if value { "true" } else { "false" },
        NamedNode::new_unchecked("http://www.w3.org/2001/XMLSchema#boolean"),
    );
    Quad::new(
        s.clone(),
        NamedNode::new_unchecked(predicate),
        lit,
        g.clone(),
    )
}
