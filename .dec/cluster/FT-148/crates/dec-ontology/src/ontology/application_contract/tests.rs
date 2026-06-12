use super::*;
use crate::ontology::provenance::{MotivationalEdge, Provenance};
use crate::vocab;
use oxrdf::{NamedNode, Quad, GraphName, Literal, Subject, Term};
use std::collections::HashMap;

#[test]
fn round_trip_application_contract() {
    let contract_id = NamedNode::new("https://decision-cli.dev/ns/test-application-contract").unwrap();
    let archetype_id = NamedNode::new("https://decision-cli.dev/ns/test-archetype").unwrap();
    let language_runtime_id = NamedNode::new("https://decision-cli.dev/ns/test-language-runtime").unwrap();
    let layering_rule_id = NamedNode::new("https://decision-cli.dev/ns/test-layering-rule").unwrap();
    let feature_organisation_id = NamedNode::new("https://decision-cli.dev/ns/test-feature-organisation").unwrap();
    let persistence_model_id = NamedNode::new("https://decision-cli.dev/ns/test-persistence-model").unwrap();
    let endpoint_convention_id = NamedNode::new("https://decision-cli.dev/ns/test-endpoint-convention").unwrap();

    let convention_subject = Subject::from(NamedNode::new("https://decision-cli.dev/ns/test-convention").unwrap());
    let convention = Convention {
        id: NamedNode::new("https://decision-cli.dev/ns/test-convention").unwrap(),
        name: "Test Convention".to_string(),
        body_path: "/path/to/test/convention".into(),
        audit_id: Some(NamedNode::new("https://decision-cli.dev/ns/test-audit").unwrap()),
        checkable: true,
    };

    let provenance = Provenance {
        was_generated_by: NamedNode::new("https://decision-cli.dev/ns/test-session").unwrap(),
        was_attributed_to: NamedNode::new("https://decision-cli.dev/ns/test-agent").unwrap(),
        generated_at_time: "2023-01-01T00:00:00Z".to_string(),
        motivational: vec![
            MotivationalEdge {
                predicate: vocab::DECISION_MOTIVATION_SOURCE,
                target: NamedNode::new("https://decision-cli.dev/ns/test-source").unwrap(),
            }
        ],
    };

    let contract = ApplicationContract {
        id: contract_id.clone(),
        archetype: archetype_id.clone(),
        language_runtime: convention.clone(),
        layering_rule: convention.clone(),
        feature_organisation: convention.clone(),
        persistence_model: convention.clone(),
        endpoint_convention: convention.clone(),
        cross_cutting: vec![convention.clone()],
        provenance: provenance.clone(),
    };

    let graph = GraphName::DefaultGraph;
    let quads = contract.to_quads(&contract_id, &graph);
    let parsed = quads_to_application_contract(&quads).unwrap();

    assert_eq!(parsed.id, contract.id);
    assert_eq!(parsed.archetype, contract.archetype);
    assert_eq!(parsed.language_runtime, contract.language_runtime);
    assert_eq!(parsed.layering_rule, contract.layering_rule);
    assert_eq!(parsed.feature_organisation, contract.feature_organisation);
    assert_eq!(parsed.persistence_model, contract.persistence_model);
    assert_eq!(parsed.endpoint_convention, contract.endpoint_convention);
    assert_eq!(parsed.cross_cutting, contract.cross_cutting);
    assert_eq!(parsed.provenance, contract.provenance);
}