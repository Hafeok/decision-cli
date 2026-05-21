//! Parse-time invariant checks for the embedded ontology bundle.

use oxigraph::io::{RdfFormat, RdfParser};
use oxigraph::model::{GraphName, NamedNode};
use oxigraph::sparql::QueryResults;
use oxigraph::store::Store;
use sha2::{Digest, Sha256};

use super::{OntologyError, ONTOLOGY_GRAPH_IRI, ONTOLOGY_TTL, SHAPES_GRAPH_IRI, SHAPES_TTL};

pub(super) fn load_turtle_into_graph(
    store: &Store,
    ttl: &str,
    graph_iri: &str,
) -> Result<(), OntologyError> {
    let graph = NamedNode::new_unchecked(graph_iri);
    let parser = RdfParser::from_format(RdfFormat::Turtle)
        .without_named_graphs()
        .with_default_graph(GraphName::NamedNode(graph));
    store
        .load_from_reader(parser, ttl.as_bytes())
        .map_err(|err| OntologyError::CompiledAssetMalformed(err.to_string()))?;
    Ok(())
}

pub(super) fn sha256_hex_of_assets() -> String {
    let mut hasher = Sha256::new();
    hasher.update(ONTOLOGY_TTL.as_bytes());
    hasher.update(b"\x00");
    hasher.update(SHAPES_TTL.as_bytes());
    let digest = hasher.finalize();
    let mut out = String::with_capacity(digest.len() * 2);
    for b in digest {
        use std::fmt::Write;
        let _ = write!(out, "{b:02x}");
    }
    out
}

pub(super) fn extract_version_info(store: &Store) -> Result<Option<String>, OntologyError> {
    let q = format!(
        "SELECT ?v WHERE {{ GRAPH <{g}> {{ <https://decision-cli.dev/ns> <http://www.w3.org/2002/07/owl#versionInfo> ?v }} }} LIMIT 1",
        g = ONTOLOGY_GRAPH_IRI
    );
    let results = store
        .query(q.as_str())
        .map_err(|err| OntologyError::CompiledAssetMalformed(err.to_string()))?;
    let QueryResults::Solutions(mut sols) = results else {
        return Err(OntologyError::CompiledAssetMalformed(
            "owl:versionInfo query returned a non-solution result".to_string(),
        ));
    };
    if let Some(sol) = sols.next() {
        let sol = sol.map_err(|err| OntologyError::CompiledAssetMalformed(err.to_string()))?;
        if let Some(term) = sol.get("v") {
            if let oxigraph::model::Term::Literal(lit) = term {
                return Ok(Some(lit.value().to_string()));
            }
        }
    }
    Ok(None)
}

pub(super) fn invariant_ontology_classes_present(store: &Store) -> Result<(), OntologyError> {
    // FT-006 §Invariants: at minimum declare ValueStream, ValueAction,
    // Goal, Session, Dispatch, Event.
    for class in [
        "ValueStream",
        "ValueAction",
        "Goal",
        "Session",
        "Dispatch",
        "Event",
        "Role",
        "Authority",
        "ActionSession",
        "InterpretationSession",
        "DispatchGroup",
        "VerificationVerdict",
        "Feedback",
        "VerificationEnvironment",
        "VerificationGraph",
        "VerificationStep",
    ] {
        let iri = format!("https://decision-cli.dev/ns#{class}");
        let q = format!(
            "ASK {{ GRAPH <{g}> {{ <{iri}> a <http://www.w3.org/2000/01/rdf-schema#Class> }} }}",
            g = ONTOLOGY_GRAPH_IRI
        );
        let result = store
            .query(q.as_str())
            .map_err(|err| OntologyError::CompiledAssetMalformed(err.to_string()))?;
        if !matches!(result, QueryResults::Boolean(true)) {
            return Err(OntologyError::InvariantViolation(format!(
                "ontology must declare dec:{class} as rdfs:Class"
            )));
        }
    }
    Ok(())
}

pub(super) fn invariant_shapes_present(store: &Store) -> Result<(), OntologyError> {
    for (target_class, props) in required_shape_properties() {
        for prop in props.iter() {
            ensure_min_count_property(store, target_class, prop)?;
        }
    }
    Ok(())
}

const VALUE_STREAM_PROPS: &[&str] = &[
    "https://decision-cli.dev/ns#name",
    "https://decision-cli.dev/ns#title",
    "https://decision-cli.dev/ns#terminalValueAction",
    "https://decision-cli.dev/ns#authorizedGoals",
];

const VALUE_ACTION_PROPS: &[&str] = &[
    "https://decision-cli.dev/ns#name",
    "https://decision-cli.dev/ns#description",
    "https://decision-cli.dev/ns#exitCriterion",
    "https://decision-cli.dev/ns#expectedOutputType",
    "https://decision-cli.dev/ns#compatibleGoals",
];

const ROLE_PROPS: &[&str] = &[
    "https://decision-cli.dev/ns#roleId",
    "https://decision-cli.dev/ns#roleInputType",
    "https://decision-cli.dev/ns#roleOutputType",
    "https://decision-cli.dev/ns#roleModelBinding",
];

const VERIFICATION_VERDICT_PROPS: &[&str] = &[
    "https://decision-cli.dev/ns#verdict",
    "https://decision-cli.dev/ns#rationale",
    "http://www.w3.org/ns/prov#wasGeneratedBy",
    "http://www.w3.org/ns/prov#used",
    "https://decision-cli.dev/ns#inStream",
];

const DISPATCH_GROUP_PROPS: &[&str] = &[
    "https://decision-cli.dev/ns#dispatchStatus",
    "https://decision-cli.dev/ns#dispatchedFor",
    "https://decision-cli.dev/ns#hasActionSession",
    "https://decision-cli.dev/ns#inStream",
];

const FEEDBACK_PROPS: &[&str] = &[
    "https://decision-cli.dev/ns#feedbackClass",
    "https://decision-cli.dev/ns#lifecycleState",
    "https://decision-cli.dev/ns#targetRole",
    "https://decision-cli.dev/ns#evidence",
    "https://decision-cli.dev/ns#sourceSession",
    "https://decision-cli.dev/ns#inStream",
];

const VERIFICATION_ENV_PROPS: &[&str] = &[
    "https://decision-cli.dev/ns#envType",
    "https://decision-cli.dev/ns#safetyClass",
    "https://decision-cli.dev/ns#allowedOps",
];

const VERIFICATION_GRAPH_PROPS: &[&str] = &[
    "https://decision-cli.dev/ns#verifies",
    "https://decision-cli.dev/ns#environment",
    "https://decision-cli.dev/ns#steps",
];

const VERIFICATION_STEP_PROPS: &[&str] = &[
    "https://decision-cli.dev/ns#stepType",
];

fn required_shape_properties() -> &'static [(&'static str, &'static [&'static str])] {
    &[
        ("https://decision-cli.dev/ns#ValueStream", VALUE_STREAM_PROPS),
        ("https://decision-cli.dev/ns#ValueAction", VALUE_ACTION_PROPS),
        ("https://decision-cli.dev/ns#Role", ROLE_PROPS),
        (
            "https://decision-cli.dev/ns#VerificationVerdict",
            VERIFICATION_VERDICT_PROPS,
        ),
        (
            "https://decision-cli.dev/ns#DispatchGroup",
            DISPATCH_GROUP_PROPS,
        ),
        ("https://decision-cli.dev/ns#Feedback", FEEDBACK_PROPS),
        (
            "https://decision-cli.dev/ns#VerificationEnvironment",
            VERIFICATION_ENV_PROPS,
        ),
        (
            "https://decision-cli.dev/ns#VerificationGraph",
            VERIFICATION_GRAPH_PROPS,
        ),
        (
            "https://decision-cli.dev/ns#VerificationStep",
            VERIFICATION_STEP_PROPS,
        ),
    ]
}

fn ensure_min_count_property(
    store: &Store,
    target_class: &str,
    prop: &str,
) -> Result<(), OntologyError> {
    let q = format!(
        "ASK {{ GRAPH <{g}> {{ \
            ?shape <http://www.w3.org/ns/shacl#targetClass> <{target_class}> ; \
                   <http://www.w3.org/ns/shacl#property> ?p . \
            ?p <http://www.w3.org/ns/shacl#path> <{prop}> ; \
               <http://www.w3.org/ns/shacl#minCount> ?_ . \
         }} }}",
        g = SHAPES_GRAPH_IRI
    );
    let result = store
        .query(q.as_str())
        .map_err(|err| OntologyError::CompiledAssetMalformed(err.to_string()))?;
    if !matches!(result, QueryResults::Boolean(true)) {
        return Err(OntologyError::InvariantViolation(format!(
            "shapes graph is missing a sh:minCount constraint on {prop} for {target_class}"
        )));
    }
    Ok(())
}
