use crate::ontology::archetype::{Archetype, ArchetypeStatus, Variance, ArchetypeEvidence};
use crate::ontology::provenance::Provenance;
use crate::vocab::archetype::*;
use oxigraph::model::{NamedNode, Quad, Subject, Term};
use std::collections::HashMap;

pub fn quads_to_archetype(quads: &[Quad]) -> Result<Archetype, String> {
    let mut id: Option<NamedNode> = None;
    let mut title: Option<String> = None;
    let mut status: Option<ArchetypeStatus> = None;
    let mut application_contract: Option<NamedNode> = None;
    let mut infrastructure_contract_template: Option<NamedNode> = None;
    let mut infrastructure_contract_instances: Vec<NamedNode> = Vec::new();
    let mut application_task_types: Vec<NamedNode> = Vec::new();
    let mut infrastructure_task_types: Vec<NamedNode> = Vec::new();
    let mut archetype_audits: Vec<NamedNode> = Vec::new();
    let mut seam_audits: Vec<NamedNode> = Vec::new();
    let mut archetype_layer_estimate: Option<f32> = None;
    let mut instance_variance: Option<Variance> = None;
    let mut application_contract_held_invariant: Option<bool> = None;
    let mut coverage_note: Option<String> = None;
    let mut provenance: Option<Provenance> = None;

    let quad_map: HashMap<Subject, Vec<Quad>> = quads.iter().fold(HashMap::new(), |mut acc, quad| {
        acc.entry(quad.subject.clone()).or_insert_with(Vec::new).push(quad.clone());
        acc
    });

    for quad in quads {
        match quad.predicate.as_ref() {
            ARCHETYPE_CLASS => {
                if id.is_none() {
                    id = Some(quad.subject.clone().into_named_node().ok_or("Invalid subject")?);
                }
            }
            ARCHETYPE_TITLE => {
                if let Some(Term::Literal(literal)) = quad.object.as_ref() {
                    title = Some(literal.value().to_string());
                } else {
                    return Err("Invalid title".to_string());
                }
            }
            ARCHETYPE_STATUS => {
                if let Some(Term::Literal(literal)) = quad.object.as_ref() {
                    status = match literal.value().as_str() {
                        "candidate" => Some(ArchetypeStatus::Candidate),
                        "standard" => Some(ArchetypeStatus::Standard),
                        "quarantined" => Some(ArchetypeStatus::Quarantined),
                        _ => return Err("Invalid status".to_string()),
                    };
                } else {
                    return Err("Invalid status".to_string());
                }
            }
            APPLICATION_CONTRACT => {
                if let Some(object) = quad.object.clone().into_named_node() {
                    application_contract = Some(object);
                } else {
                    return Err("Invalid application contract".to_string());
                }
            }
            INFRASTRUCTURE_CONTRACT_TEMPLATE => {
                if let Some(object) = quad.object.clone().into_named_node() {
                    infrastructure_contract_template = Some(object);
                } else {
                    return Err("Invalid infrastructure contract template".to_string());
                }
            }
            INFRASTRUCTURE_CONTRACT_INSTANCES => {
                if let Some(object) = quad.object.clone().into_named_node() {
                    infrastructure_contract_instances.push(object);
                } else {
                    return Err("Invalid infrastructure contract instance".to_string());
                }
            }
            APPLICATION_TASK_TYPES => {
                if let Some(object) = quad.object.clone().into_named_node() {
                    application_task_types.push(object);
                } else {
                    return Err("Invalid application task type".to_string());
                }
            }
            INFRASTRUCTURE_TASK_TYPES => {
                if let Some(object) = quad.object.clone().into_named_node() {
                    infrastructure_task_types.push(object);
                } else {
                    return Err("Invalid infrastructure task type".to_string());
                }
            }
            ARCHETYPE_AUDITS => {
                if let Some(object) = quad.object.clone().into_named_node() {
                    archetype_audits.push(object);
                } else {
                    return Err("Invalid archetype audit".to_string());
                }
            }
            SEAM_AUDITS => {
                if let Some(object) = quad.object.clone().into_named_node() {
                    seam_audits.push(object);
                } else {
                    return Err("Invalid seam audit".to_string());
                }
            }
            ARCHETYPE_LAYER_ESTIMATE => {
                if let Some(Term::Literal(literal)) = quad.object.as_ref() {
                    archetype_layer_estimate = Some(literal.value().parse::<f32>().map_err(|_| "Invalid archetype layer estimate")?);
                } else {
                    return Err("Invalid archetype layer estimate".to_string());
                }
            }
            INSTANCE_VARIANCE => {
                if let Some(Term::Literal(literal)) = quad.object.as_ref() {
                    instance_variance = match literal.value().as_str() {
                        "low" => Some(Variance::Low),
                        "medium" => Some(Variance::Medium),
                        "high" => Some(Variance::High),
                        _ => return Err("Invalid instance variance".to_string()),
                    };
                } else {
                    return Err("Invalid instance variance".to_string());
                }
            }
            APPLICATION_CONTRACT_HELD_INVARIANT => {
                if let Some(Term::Literal(literal)) = quad.object.as_ref() {
                    application_contract_held_invariant = Some(literal.value().parse::<bool>().map_err(|_| "Invalid application contract held invariant")?);
                } else {
                    return Err("Invalid application contract held invariant".to_string());
                }
            }
            COVERAGE_NOTE => {
                if let Some(Term::Literal(literal)) = quad.object.as_ref() {
                    coverage_note = Some(literal.value().to_string());
                } else {
                    return Err("Invalid coverage note".to_string());
                }
            }
            _ => {
                // Handle provenance predicates
                if provenance.is_none() {
                    provenance = Some(Provenance::from_quads(&quad_map, &quad.subject));
                }
            }
        }
    }

    // Validate required fields
    let id = id.ok_or("Missing ID")?;
    let title = title.ok_or("Missing title")?;
    let status = status.ok_or("Missing status")?;
    let application_contract = application_contract.ok_or("Missing application contract")?;
    let infrastructure_contract_template = infrastructure_contract_template.ok_or("Missing infrastructure contract template")?;
    let archetype_layer_estimate = archetype_layer_estimate.ok_or("Missing archetype layer estimate")?;
    let instance_variance = instance_variance.ok_or("Missing instance variance")?;
    let application_contract_held_invariant = application_contract_held_invariant.ok_or("Missing application contract held invariant")?;
    let coverage_note = coverage_note.ok_or("Missing coverage note")?;

    let evidence = ArchetypeEvidence {
        archetype_layer_estimate,
        instance_variance,
        application_contract_held_invariant,
        coverage_note,
    };

    Ok(Archetype {
        id,
        title,
        status,
        application_contract,
        infrastructure_contract_template,
        infrastructure_contract_instances,
        application_task_types,
        infrastructure_task_types,
        archetype_audits,
        seam_audits,
        evidence,
        provenance: provenance.unwrap_or_default(),
    })
}