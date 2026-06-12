use crate::ontology::application_contract::ApplicationContract;
use crate::ontology::provenance::{MotivationalEdge, Provenance};
use crate::vocab;
use oxrdf::{NamedNode, Quad, GraphName, Subject, Literal};
use std::collections::HashMap;

/// Negative test cases for ApplicationContract parsing.
#[cfg(test)]
mod tests {
    use super::*;
    use crate::ontology::application_contract::Convention;
    use crate::ontology::archetype::parser::ArchetypeParseError;
    use std::path::PathBuf;

    #[test]
    fn test_missing_id() {
        let mut quads = vec![];
        let graph = GraphName::default();
        let subject = NamedNode::new("https://decision-cli.dev/ns/test-application-contract").unwrap();

        // Add a minimal valid ApplicationContract structure
        quads.push(Quad::new(
            subject.clone().into(),
            vocab::ARCHETYPE.into(),
            NamedNode::new("https://decision-cli.dev/ns/test-archetype").unwrap().into(),
            graph.clone()
        ));
        quads.push(Quad::new(
            subject.clone().into(),
            vocab::LANGUAGE_RUNTIME.into(),
            NamedNode::new("https://decision-cli.dev/ns/test-language-runtime").unwrap().into(),
            graph.clone()
        ));
        quads.push(Quad::new(
            subject.clone().into(),
            vocab::LAYERING_RULE.into(),
            NamedNode::new("https://decision-cli.dev/ns/test-layering-rule").unwrap().into(),
            graph.clone()
        ));
        quads.push(Quad::new(
            subject.clone().into(),
            vocab::FEATURE_ORGANISATION.into(),
            NamedNode::new("https://decision-cli.dev/ns/test-feature-org").unwrap().into(),
            graph.clone()
        ));
        quads.push(Quad::new(
            subject.clone().into(),
            vocab::PERSISTENCE_MODEL.into(),
            NamedNode::new("https://decision-cli.dev/ns/test-persistence-model").unwrap().into(),
            graph.clone()
        ));
        quads.push(Quad::new(
            subject.clone().into(),
            vocab::ENDPOINT_CONVENTION.into(),
            NamedNode::new("https://decision-cli.dev/ns/test-endpoint").unwrap().into(),
            graph.clone()
        ));
        quads.push(Quad::new(
            subject.clone().into(),
            vocab::PROVENANCE.into(),
            NamedNode::new("https://decision-cli.dev/ns/test-provenance").unwrap().into(),
            graph.clone()
        ));

        // Try to parse without the required id field
        assert!(matches!(
            crate::ontology::application_contract::parser::quads_to_application_contract(&quads),
            Err(_)
        ));
    }

    #[test]
    fn test_missing_archetype() {
        let mut quads = vec![];
        let graph = GraphName::default();
        let subject = NamedNode::new("https://decision-cli.dev/ns/test-application-contract").unwrap();

        quads.push(Quad::new(
            subject.clone().into(),
            vocab::ID.into(),
            Literal::new_simple_literal("test-id").into(),
            graph.clone()
        ));
        quads.push(Quad::new(
            subject.clone().into(),
            vocab::LANGUAGE_RUNTIME.into(),
            NamedNode::new("https://decision-cli.dev/ns/test-language-runtime").unwrap().into(),
            graph.clone()
        ));
        quads.push(Quad::new(
            subject.clone().into(),
            vocab::LAYERING_RULE.into(),
            NamedNode::new("https://decision-cli.dev/ns/test-layering-rule").unwrap().into(),
            graph.clone()
        ));
        quads.push(Quad::new(
            subject.clone().into(),
            vocab::FEATURE_ORGANISATION.into(),
            NamedNode::new("https://decision-cli.dev/ns/test-feature-org").unwrap().into(),
            graph.clone()
        ));
        quads.push(Quad::new(
            subject.clone().into(),
            vocab::PERSISTENCE_MODEL.into(),
            NamedNode::new("https://decision-cli.dev/ns/test-persistence-model").unwrap().into(),
            graph.clone()
        ));
        quads.push(Quad::new(
            subject.clone().into(),
            vocab::ENDPOINT_CONVENTION.into(),
            NamedNode::new("https://decision-cli.dev/ns/test-endpoint").unwrap().into(),
            graph.clone()
        ));
        quads.push(Quad::new(
            subject.clone().into(),
            vocab::PROVENANCE.into(),
            NamedNode::new("https://decision-cli.dev/ns/test-provenance").unwrap().into(),
            graph.clone()
        ));

        assert!(matches!(
            crate::ontology::application_contract::parser::quads_to_application_contract(&quads),
            Err(_)
        ));
    }

    #[test]
    fn test_missing_language_runtime() {
        let mut quads = vec![];
        let graph = GraphName::default();
        let subject = NamedNode::new("https://decision-cli.dev/ns/test-application-contract").unwrap();

        quads.push(Quad::new(
            subject.clone().into(),
            vocab::ID.into(),
            Literal::new_simple_literal("test-id").into(),
            graph.clone()
        ));
        quads.push(Quad::new(
            subject.clone().into(),
            vocab::ARCHETYPE.into(),
            NamedNode::new("https://decision-cli.dev/ns/test-archetype").unwrap().into(),
            graph.clone()
        ));
        quads.push(Quad::new(
            subject.clone().into(),
            vocab::LAYERING_RULE.into(),
            NamedNode::new("https://decision-cli.dev/ns/test-layering-rule").unwrap().into(),
            graph.clone()
        ));
        quads.push(Quad::new(
            subject.clone().into(),
            vocab::FEATURE_ORGANISATION.into(),
            NamedNode::new("https://decision-cli.dev/ns/test-feature-org").unwrap().into(),
            graph.clone()
        ));
        quads.push(Quad::new(
            subject.clone().into(),
            vocab::PERSISTENCE_MODEL.into(),
            NamedNode::new("https://decision-cli.dev/ns/test-persistence-model").unwrap().into(),
            graph.clone()
        ));
        quads.push(Quad::new(
            subject.clone().into(),
            vocab::ENDPOINT_CONVENTION.into(),
            NamedNode::new("https://decision-cli.dev/ns/test-endpoint").unwrap().into(),
            graph.clone()
        ));
        quads.push(Quad::new(
            subject.clone().into(),
            vocab::PROVENANCE.into(),
            NamedNode::new("https://decision-cli.dev/ns/test-provenance").unwrap().into(),
            graph.clone()
        ));

        assert!(matches!(
            crate::ontology::application_contract::parser::quads_to_application_contract(&quads),
            Err(_)
        ));
    }

    #[test]
    fn test_missing_layering_rule() {
        let mut quads = vec![];
        let graph = GraphName::default();
        let subject = NamedNode::new("https://decision-cli.dev/ns/test-application-contract").unwrap();

        quads.push(Quad::new(
            subject.clone().into(),
            vocab::ID.into(),
            Literal::new_simple_literal("test-id").into(),
            graph.clone()
        ));
        quads.push(Quad::new(
            subject.clone().into(),
            vocab::ARCHETYPE.into(),
            NamedNode::new("https://decision-cli.dev/ns/test-archetype").unwrap().into(),
            graph.clone()
        ));
        quads.push(Quad::new(
            subject.clone().into(),
            vocab::LANGUAGE_RUNTIME.into(),
            NamedNode::new("https://decision-cli.dev/ns/test-language-runtime").unwrap().into(),
            graph.clone()
        ));
        quads.push(Quad::new(
            subject.clone().into(),
            vocab::FEATURE_ORGANISATION.into(),
            NamedNode::new("https://decision-cli.dev/ns/test-feature-org").unwrap().into(),
            graph.clone()
        ));
        quads.push(Quad::new(
            subject.clone().into(),
            vocab::PERSISTENCE_MODEL.into(),
            NamedNode::new("https://decision-cli.dev/ns/test-persistence-model").unwrap().into(),
            graph.clone()
        ));
        quads.push(Quad::new(
            subject.clone().into(),
            vocab::ENDPOINT_CONVENTION.into(),
            NamedNode::new("https://decision-cli.dev/ns/test-endpoint").unwrap().into(),
            graph.clone()
        ));
        quads.push(Quad::new(
            subject.clone().into(),
            vocab::PROVENANCE.into(),
            NamedNode::new("https://decision-cli.dev/ns/test-provenance").unwrap().into(),
            graph.clone()
        ));

        assert!(matches!(
            crate::ontology::application_contract::parser::quads_to_application_contract(&quads),
            Err(_)
        ));
    }

    #[test]
    fn test_missing_feature_organisation() {
        let mut quads = vec![];
        let graph = GraphName::default();
        let subject = NamedNode::new("https://decision-cli.dev/ns/test-application-contract").unwrap();

        quads.push(Quad::new(
            subject.clone().into(),
            vocab::ID.into(),
            Literal::new_simple_literal("test-id").into(),
            graph.clone()
        ));
        quads.push(Quad::new(
            subject.clone().into(),
            vocab::ARCHETYPE.into(),
            NamedNode::new("https://decision-cli.dev/ns/test-archetype").unwrap().into(),
            graph.clone()
        ));
        quads.push(Quad::new(
            subject.clone().into(),
            vocab::LANGUAGE_RUNTIME.into(),
            NamedNode::new("https://decision-cli.dev/ns/test-language-runtime").unwrap().into(),
            graph.clone()
        ));
        quads.push(Quad::new(
            subject.clone().into(),
            vocab::LAYERING_RULE.into(),
            NamedNode::new("https://decision-cli.dev/ns/test-layering-rule").unwrap().into(),
            graph.clone()
        ));
        quads.push(Quad::new(
            subject.clone().into(),
            vocab::PERSISTENCE_MODEL.into(),
            NamedNode::new("https://decision-cli.dev/ns/test-persistence-model").unwrap().into(),
            graph.clone()
        ));
        quads.push(Quad::new(
            subject.clone().into(),
            vocab::ENDPOINT_CONVENTION.into(),
            NamedNode::new("https://decision-cli.dev/ns/test-endpoint").unwrap().into(),
            graph.clone()
        ));
        quads.push(Quad::new(
            subject.clone().into(),
            vocab::PROVENANCE.into(),
            NamedNode::new("https://decision-cli.dev/ns/test-provenance").unwrap().into(),
            graph.clone()
        ));

        assert!(matches!(
            crate::ontology::application_contract::parser::quads_to_application_contract(&quads),
            Err(_)
        ));
    }

    #[test]
    fn test_missing_persistence_model() {
        let mut quads = vec![];
        let graph = GraphName::default();
        let subject = NamedNode::new("https://decision-cli.dev/ns/test-application-contract").unwrap();

        quads.push(Quad::new(
            subject.clone().into(),
            vocab::ID.into(),
            Literal::new_simple_literal("test-id").into(),
            graph.clone()
        ));
        quads.push(Quad::new(
            subject.clone().into(),
            vocab::ARCHETYPE.into(),
            NamedNode::new("https://decision-cli.dev/ns/test-archetype").unwrap().into(),
            graph.clone()
        ));
        quads.push(Quad::new(
            subject.clone().into(),
            vocab::LANGUAGE_RUNTIME.into(),
            NamedNode::new("https://decision-cli.dev/ns/test-language-runtime").unwrap().into(),
            graph.clone()
        ));
        quads.push(Quad::new(
            subject.clone().into(),
            vocab::LAYERING_RULE.into(),
            NamedNode::new("https://decision-cli.dev/ns/test-layering-rule").unwrap().into(),
            graph.clone()
        ));
        quads.push(Quad::new(
            subject.clone().into(),
            vocab::FEATURE_ORGANISATION.into(),
            NamedNode::new("https://decision-cli.dev/ns/test-feature-org").unwrap().into(),
            graph.clone()
        ));
        quads.push(Quad::new(
            subject.clone().into(),
            vocab::ENDPOINT_CONVENTION.into(),
            NamedNode::new("https://decision-cli.dev/ns/test-endpoint").unwrap().into(),
            graph.clone()
        ));
        quads.push(Quad::new(
            subject.clone().into(),
            vocab::PROVENANCE.into(),
            NamedNode::new("https://decision-cli.dev/ns/test-provenance").unwrap().into(),
            graph.clone()
        ));

        assert!(matches!(
            crate::ontology::application_contract::parser::quads_to_application_contract(&quads),
            Err(_)
        ));
    }

    #[test]
    fn test_missing_endpoint_convention() {
        let mut quads = vec![];
        let graph = GraphName::default();
        let subject = NamedNode::new("https://decision-cli.dev/ns/test-application-contract").unwrap();

        quads.push(Quad::new(
            subject.clone().into(),
            vocab::ID.into(),
            Literal::new_simple_literal("test-id").into(),
            graph.clone()
        ));
        quads.push(Quad::new(
            subject.clone().into(),
            vocab::ARCHETYPE.into(),
            NamedNode::new("https://decision-cli.dev/ns/test-archetype").unwrap().into(),
            graph.clone()
        ));
        quads.push(Quad::new(
            subject.clone().into(),
            vocab::LANGUAGE_RUNTIME.into(),
            NamedNode::new("https://decision-cli.dev/ns/test-language-runtime").unwrap().into(),
            graph.clone()
        ));
        quads.push(Quad::new(
            subject.clone().into(),
            vocab::LAYERING_RULE.into(),
            NamedNode::new("https://decision-cli.dev/ns/test-layering-rule").unwrap().into(),
            graph.clone()
        ));
        quads.push(Quad::new(
            subject.clone().into(),
            vocab::FEATURE_ORGANISATION.into(),
            NamedNode::new("https://decision-cli.dev/ns/test-feature-org").unwrap().into(),
            graph.clone()
        ));
        quads.push(Quad::new(
            subject.clone().into(),
            vocab::PERSISTENCE_MODEL.into(),
            NamedNode::new("https://decision-cli.dev/ns/test-persistence-model").unwrap().into(),
            graph.clone()
        ));
        quads.push(Quad::new(
            subject.clone().into(),
            vocab::PROVENANCE.into(),
            NamedNode::new("https://decision-cli.dev/ns/test-provenance").unwrap().into(),
            graph.clone()
        ));

        assert!(matches!(
            crate::ontology::application_contract::parser::quads_to_application_contract(&quads),
            Err(_)
        ));
    }

    #[test]
    fn test_missing_provenance() {
        let mut quads = vec![];
        let graph = GraphName::default();
        let subject = NamedNode::new("https://decision-cli.dev/ns/test-application-contract").unwrap();

        quads.push(Quad::new(
            subject.clone().into(),
            vocab::ID.into(),
            Literal::new_simple_literal("test-id").into(),
            graph.clone()
        ));
        quads.push(Quad::new(
            subject.clone().into(),
            vocab::ARCHETYPE.into(),
            NamedNode::new("https://decision-cli.dev/ns/test-archetype").unwrap().into(),
            graph.clone()
        ));
        quads.push(Quad::new(
            subject.clone().into(),
            vocab::LANGUAGE_RUNTIME.into(),
            NamedNode::new("https://decision-cli.dev/ns/test-language-runtime").unwrap().into(),
            graph.clone()
        ));
        quads.push(Quad::new(
            subject.clone().into(),
            vocab::LAYERING_RULE.into(),
            NamedNode::new("https://decision-cli.dev/ns/test-layering-rule").unwrap().into(),
            graph.clone()
        ));
        quads.push(Quad::new(
            subject.clone().into(),
            vocab::FEATURE_ORGANISATION.into(),
            NamedNode::new("https://decision-cli.dev/ns/test-feature-org").unwrap().into(),
            graph.clone()
        ));
        quads.push(Quad::new(
            subject.clone().into(),
            vocab::PERSISTENCE_MODEL.into(),
            NamedNode::new("https://decision-cli.dev/ns/test-persistence-model").unwrap().into(),
            graph.clone()
        ));
        quads.push(Quad::new(
            subject.clone().into(),
            vocab::ENDPOINT_CONVENTION.into(),
            NamedNode::new("https://decision-cli.dev/ns/test-endpoint").unwrap().into(),
            graph.clone()
        ));

        assert!(matches!(
            crate::ontology::application_contract::parser::quads_to_application_contract(&quads),
            Err(_)
        ));
    }
}