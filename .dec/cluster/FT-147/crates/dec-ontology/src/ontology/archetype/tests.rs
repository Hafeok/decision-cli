use crate::ontology::archetype::{Archetype, ArchetypeStatus, Variance, ArchetypeEvidence};
use crate::ontology::provenance::Provenance;
use crate::vocab::archetype::*;
use oxigraph::model::{NamedNode, Quad, Subject, Literal, Iri};
use std::collections::HashMap;

#[test]
fn test_round_trip() {
    let original_archetype = Archetype {
        id: NamedNode::new("http://example.org/archetype/test").unwrap(),
        title: "Test Archetype".to_string(),
        status: ArchetypeStatus::Candidate,
        application_contract: NamedNode::new("http://example.org/contract/application").unwrap(),
        infrastructure_contract_template: NamedNode::new("http://example.org/contract/infrastructure-template").unwrap(),
        infrastructure_contract_instances: vec![
            NamedNode::new("http://example.org/contract/infrastructure-instance-1").unwrap(),
            NamedNode::new("http://example.org/contract/infrastructure-instance-2").unwrap(),
        ],
        application_task_types: vec![
            NamedNode::new("http://example.org/task/application-task-1").unwrap(),
            NamedNode::new("http://example.org/task/application-task-2").unwrap(),
        ],
        infrastructure_task_types: vec![
            NamedNode::new("http://example.org/task/infrastructure-task-1").unwrap(),
            NamedNode::new("http://example.org/task/infrastructure-task-2").unwrap(),
        ],
        archetype_audits: vec![
            NamedNode::new("http://example.org/audit/archetype-audit-1").unwrap(),
            NamedNode::new("http://example.org/audit/archetype-audit-2").unwrap(),
        ],
        seam_audits: vec![
            NamedNode::new("http://example.org/audit/seam-audit-1").unwrap(),
            NamedNode::new("http://example.org/audit/seam-audit-2").unwrap(),
            NamedNode::new("http://example.org/audit/seam-audit-3").unwrap(),
        ],
        evidence: ArchetypeEvidence {
            archetype_layer_estimate: 0.85,
            instance_variance: Variance::Medium,
            application_contract_held_invariant: true,
            coverage_note: "Test coverage note".to_string(),
        },
        provenance: Provenance::default(),
    };

    let quads = original_archetype.to_quads();
    let parsed_archetype = quads_to_archetype(&quads).expect("Failed to parse archetype");
    
    assert_eq!(original_archetype, parsed_archetype);
}

#[test]
fn test_seam_audits_empty_should_fail() {
    let archetype = Archetype {
        id: NamedNode::new("http://example.org/archetype/test").unwrap(),
        title: "Test Archetype".to_string(),
        status: ArchetypeStatus::Candidate,
        application_contract: NamedNode::new("http://example.org/contract/application").unwrap(),
        infrastructure_contract_template: NamedNode::new("http://example.org/contract/infrastructure-template").unwrap(),
        infrastructure_contract_instances: vec![],
        application_task_types: vec![],
        infrastructure_task_types: vec![],
        archetype_audits: vec![],
        seam_audits: vec![], // Empty seam audits
        evidence: ArchetypeEvidence {
            archetype_layer_estimate: 0.85,
            instance_variance: Variance::Medium,
            application_contract_held_invariant: true,
            coverage_note: "Test coverage note".to_string(),
        },
        provenance: Provenance::default(),
    };

    let quads = archetype.to_quads();
    let result = quads_to_archetype(&quads);
    assert!(result.is_err());
}

#[test]
fn test_invalid_status_should_fail() {
    let archetype = Archetype {
        id: NamedNode::new("http://example.org/archetype/test").unwrap(),
        title: "Test Archetype".to_string(),
        status: ArchetypeStatus::Candidate,
        application_contract: NamedNode::new("http://example.org/contract/application").unwrap(),
        infrastructure_contract_template: NamedNode::new("http://example.org/contract/infrastructure-template").unwrap(),
        infrastructure_contract_instances: vec![],
        application_task_types: vec![],
        infrastructure_task_types: vec![],
        archetype_audits: vec![],
        seam_audits: vec![
            NamedNode::new("http://example.org/audit/seam-audit-1").unwrap(),
        ],
        evidence: ArchetypeEvidence {
            archetype_layer_estimate: 0.85,
            instance_variance: Variance::Medium,
            application_contract_held_invariant: true,
            coverage_note: "Test coverage note".to_string(),
        },
        provenance: Provenance::default(),
    };

    // Manually create invalid quad with wrong status
    let mut quads = archetype.to_quads();
    // Find and replace the status quad with an invalid value
    for i in 0..quads.len() {
        if quads[i].predicate == ARCHETYPE_STATUS.into() {
            quads[i] = Quad::new(
                archetype.id.clone(),
                ARCHETYPE_STATUS.into(),
                "invalid_status".into(),
                None,
            );
            break;
        }
    }

    let result = quads_to_archetype(&quads);
    assert!(result.is_err());
}

#[test]
fn test_missing_application_contract_should_fail() {
    let archetype = Archetype {
        id: NamedNode::new("http://example.org/archetype/test").unwrap(),
        title: "Test Archetype".to_string(),
        status: ArchetypeStatus::Candidate,
        application_contract: NamedNode::new("http://example.org/contract/application").unwrap(),
        infrastructure_contract_template: NamedNode::new("http://example.org/contract/infrastructure-template").unwrap(),
        infrastructure_contract_instances: vec![],
        application_task_types: vec![],
        infrastructure_task_types: vec![],
        archetype_audits: vec![],
        seam_audits: vec![
            NamedNode::new("http://example.org/audit/seam-audit-1").unwrap(),
        ],
        evidence: ArchetypeEvidence {
            archetype_layer_estimate: 0.85,
            instance_variance: Variance::Medium,
            application_contract_held_invariant: true,
            coverage_note: "Test coverage note".to_string(),
        },
        provenance: Provenance::default(),
    };

    // Manually remove the application contract quad
    let mut quads = archetype.to_quads();
    quads.retain(|q| q.predicate != APPLICATION_CONTRACT.into());

    let result = quads_to_archetype(&quads);
    assert!(result.is_err());
}