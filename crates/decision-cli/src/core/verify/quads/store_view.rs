//! Quad-and-store inspection helpers used by the FT-037 safety
//! integration: reconstructs typed `VerificationStep` and
//! `VerificationEnvironment` views from the union of an in-flight
//! insert quad-set and the store's current contents.

use std::collections::BTreeMap;

use oxigraph::model::{NamedNode, Quad, Subject, Term};
use oxigraph::store::Store;

use crate::core::ontology::verification_env::{SafetyClass, VerificationEnvironment};
use crate::core::ontology::verification_graph::{StepFields, StepKind, VerificationStep};
use crate::core::vocab::{
    IRI_DEC_ALLOWED_OPS, IRI_DEC_BIND_AS, IRI_DEC_COMMAND, IRI_DEC_CONDITION, IRI_DEC_ENDPOINT,
    IRI_DEC_ENV_TYPE, IRI_DEC_FIXTURE_SOURCE, IRI_DEC_METHOD, IRI_DEC_PATH, IRI_DEC_QUERY,
    IRI_DEC_SAFETY_CLASS, IRI_DEC_SETUP, IRI_DEC_STEP_TYPE, IRI_DEC_TARGET, IRI_DEC_TEARDOWN,
    IRI_DEC_TIMEOUT, IRI_DEC_URL,
};

use super::super::safety::{OpSource, SafetyError};

const RDF_FIRST: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#first";
const RDF_REST: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#rest";
const RDF_NIL: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#nil";

pub(super) fn step_from_quads(
    inserts: &[Quad],
    store: &Store,
    subject: &NamedNode,
) -> Result<VerificationStep, SafetyError> {
    let kind_str = literal_for(inserts, store, subject, IRI_DEC_STEP_TYPE)
        .ok_or_else(|| missing_unknownop("dec:stepType"))?;
    let kind = StepKind::parse(&kind_str)
        .ok_or_else(|| missing_unknownop("dec:stepType (unknown kind)"))?;
    let fields = build_step_fields(inserts, store, subject, kind);
    Ok(VerificationStep {
        id: subject.clone(),
        kind,
        fields,
        provides_evidence_for: Vec::new(),
    })
}

fn build_step_fields(
    inserts: &[Quad],
    store: &Store,
    subject: &NamedNode,
    kind: StepKind,
) -> StepFields {
    match kind {
        StepKind::ShellCommand => shell_command_fields(inserts, store, subject),
        StepKind::SparqlAssertion => sparql_assertion_fields(inserts, store, subject),
        StepKind::FileAssertion => file_assertion_fields(inserts, store, subject),
        StepKind::HttpRequest => http_request_fields(inserts, store, subject),
        StepKind::WaitFor => wait_for_fields(inserts, store, subject),
        StepKind::Capture => capture_fields(inserts, store, subject),
    }
}

fn shell_command_fields(inserts: &[Quad], store: &Store, subject: &NamedNode) -> StepFields {
    StepFields::ShellCommand {
        command: literal_for(inserts, store, subject, IRI_DEC_COMMAND).unwrap_or_default(),
        expect_exit_code: None,
        capture_output: None,
    }
}

fn sparql_assertion_fields(inserts: &[Quad], store: &Store, subject: &NamedNode) -> StepFields {
    StepFields::SparqlAssertion {
        target: literal_for(inserts, store, subject, IRI_DEC_TARGET).unwrap_or_default(),
        query: literal_for(inserts, store, subject, IRI_DEC_QUERY).unwrap_or_default(),
        expect_rows: None,
    }
}

fn file_assertion_fields(inserts: &[Quad], store: &Store, subject: &NamedNode) -> StepFields {
    StepFields::FileAssertion {
        path: literal_for(inserts, store, subject, IRI_DEC_PATH).unwrap_or_default(),
        expect_hash: None,
        expect_content: None,
    }
}

fn http_request_fields(inserts: &[Quad], store: &Store, subject: &NamedNode) -> StepFields {
    StepFields::HttpRequest {
        method: literal_for(inserts, store, subject, IRI_DEC_METHOD).unwrap_or_default(),
        url: literal_for(inserts, store, subject, IRI_DEC_URL).unwrap_or_default(),
        expect_status: None,
    }
}

fn wait_for_fields(inserts: &[Quad], store: &Store, subject: &NamedNode) -> StepFields {
    StepFields::WaitFor {
        condition: iri_for(inserts, store, subject, IRI_DEC_CONDITION)
            .unwrap_or_else(|| NamedNode::new_unchecked("urn:safety:unresolved-condition")),
        timeout: literal_for(inserts, store, subject, IRI_DEC_TIMEOUT).unwrap_or_default(),
    }
}

fn capture_fields(inserts: &[Quad], store: &Store, subject: &NamedNode) -> StepFields {
    StepFields::Capture {
        from_step: None,
        bind_as: literal_for(inserts, store, subject, IRI_DEC_BIND_AS).unwrap_or_default(),
    }
}

fn missing_unknownop(field: &str) -> SafetyError {
    SafetyError::UnknownOp {
        token: format!("<malformed step missing {field}>"),
        source: OpSource::Step,
    }
}

pub(super) fn load_env(
    inserts: &[Quad],
    store: &Store,
    env_iri: &NamedNode,
) -> Option<VerificationEnvironment> {
    let id = env_iri
        .as_str()
        .rsplit('/')
        .next()
        .unwrap_or("ENV-unknown")
        .to_string();
    let env_type = literal_for(inserts, store, env_iri, IRI_DEC_ENV_TYPE)?;
    let safety_class = literal_for(inserts, store, env_iri, IRI_DEC_SAFETY_CLASS)
        .and_then(|s| SafetyClass::parse(&s))?;
    let setup = literal_for(inserts, store, env_iri, IRI_DEC_SETUP);
    let teardown = literal_for(inserts, store, env_iri, IRI_DEC_TEARDOWN);
    let endpoint = literal_for(inserts, store, env_iri, IRI_DEC_ENDPOINT);
    let fixture_source = literal_for(inserts, store, env_iri, IRI_DEC_FIXTURE_SOURCE);
    let allowed_ops = list_for(inserts, store, env_iri, IRI_DEC_ALLOWED_OPS);
    Some(VerificationEnvironment {
        id,
        env_type,
        setup,
        teardown,
        allowed_ops,
        safety_class,
        endpoint,
        fixture_source,
    })
}

fn literal_for(
    inserts: &[Quad],
    store: &Store,
    subject: &NamedNode,
    predicate: &str,
) -> Option<String> {
    for q in inserts {
        if q.predicate.as_str() != predicate {
            continue;
        }
        if !matches_subject(q, subject) {
            continue;
        }
        if let Term::Literal(lit) = &q.object {
            return Some(lit.value().to_string());
        }
    }
    let pred = NamedNode::new_unchecked(predicate);
    for quad in store
        .quads_for_pattern(
            Some(Subject::NamedNode(subject.clone()).as_ref()),
            Some(pred.as_ref()),
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

fn iri_for(
    inserts: &[Quad],
    store: &Store,
    subject: &NamedNode,
    predicate: &str,
) -> Option<NamedNode> {
    for q in inserts {
        if q.predicate.as_str() != predicate {
            continue;
        }
        if !matches_subject(q, subject) {
            continue;
        }
        if let Term::NamedNode(n) = &q.object {
            return Some(n.clone());
        }
    }
    let pred = NamedNode::new_unchecked(predicate);
    for quad in store
        .quads_for_pattern(
            Some(Subject::NamedNode(subject.clone()).as_ref()),
            Some(pred.as_ref()),
            None,
            None,
        )
        .filter_map(Result::ok)
    {
        if let Term::NamedNode(n) = quad.object {
            return Some(n);
        }
    }
    None
}

fn list_for(inserts: &[Quad], store: &Store, subject: &NamedNode, predicate: &str) -> Vec<String> {
    let mut heads = list_heads_from_inserts(inserts, subject, predicate);
    if heads.is_empty() {
        heads = list_heads_from_store(store, subject, predicate);
    }
    let Some(head) = heads.into_iter().next() else {
        return Vec::new();
    };
    walk_list_to_strings(inserts, store, head)
}

fn list_heads_from_inserts(inserts: &[Quad], subject: &NamedNode, predicate: &str) -> Vec<Term> {
    inserts
        .iter()
        .filter_map(|q| {
            if q.predicate.as_str() != predicate {
                return None;
            }
            if !matches_subject(q, subject) {
                return None;
            }
            Some(q.object.clone())
        })
        .collect()
}

fn list_heads_from_store(store: &Store, subject: &NamedNode, predicate: &str) -> Vec<Term> {
    let pred = NamedNode::new_unchecked(predicate);
    let mut heads = Vec::new();
    for quad in store
        .quads_for_pattern(
            Some(Subject::NamedNode(subject.clone()).as_ref()),
            Some(pred.as_ref()),
            None,
            None,
        )
        .filter_map(Result::ok)
    {
        heads.push(quad.object);
    }
    heads
}

fn walk_list_to_strings(inserts: &[Quad], store: &Store, head: Term) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut current = head;
    let mut visited: BTreeMap<String, ()> = BTreeMap::new();
    for _ in 0..1024 {
        let key = match &current {
            Term::NamedNode(n) => format!("iri:{}", n.as_str()),
            Term::BlankNode(b) => format!("bn:{}", b.as_str()),
            _ => return out,
        };
        if visited.contains_key(&key) {
            return out;
        }
        visited.insert(key, ());
        if let Term::NamedNode(n) = &current {
            if n.as_str() == RDF_NIL {
                return out;
            }
        }
        if let Some(Term::Literal(lit)) = find_list_value(inserts, store, &current, RDF_FIRST) {
            out.push(lit.value().to_string());
        }
        let rest = find_list_value(inserts, store, &current, RDF_REST);
        match rest {
            Some(Term::NamedNode(n)) if n.as_str() == RDF_NIL => return out,
            Some(t) => current = t,
            None => return out,
        }
    }
    out
}

fn find_list_value(inserts: &[Quad], store: &Store, head: &Term, predicate: &str) -> Option<Term> {
    for q in inserts {
        if q.predicate.as_str() != predicate {
            continue;
        }
        if term_matches_subject(&q.subject, head) {
            return Some(q.object.clone());
        }
    }
    let head_subject: Subject = match head {
        Term::NamedNode(n) => n.clone().into(),
        Term::BlankNode(b) => b.clone().into(),
        _ => return None,
    };
    let pred = NamedNode::new_unchecked(predicate);
    for quad in store
        .quads_for_pattern(Some(head_subject.as_ref()), Some(pred.as_ref()), None, None)
        .filter_map(Result::ok)
    {
        return Some(quad.object);
    }
    None
}

fn term_matches_subject(s: &Subject, t: &Term) -> bool {
    match (s, t) {
        (Subject::NamedNode(a), Term::NamedNode(b)) => a == b,
        (Subject::BlankNode(a), Term::BlankNode(b)) => a == b,
        _ => false,
    }
}

fn matches_subject(q: &Quad, s: &NamedNode) -> bool {
    matches!(&q.subject, Subject::NamedNode(n) if n == s)
}
