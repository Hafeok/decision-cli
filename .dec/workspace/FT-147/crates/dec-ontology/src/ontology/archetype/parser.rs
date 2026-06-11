//! Parser for Archetype artifact type.

use crate::ontology::archetype::{Archetype, ArchetypeStatus, ArchetypeEvidence, Variance, Provenance};
use crate::vocab::archetype::*;
use oxigraph::model::{NamedNode, Quad, Subject, Graph};
use std::collections::HashMap;

/// Parse an Archetype from a collection of quads.
pub fn parse_archetype(quads: &[Quad]) -> Result<Archetype, String> {
    let mut quad_map: HashMap<Subject, Vec<Quad>> = HashMap::new();
    
    for quad in quads {
        quad_map.entry(quad.subject.clone()).or_default().push(quad.clone());
    }
    
    // Extract the archetype IRI
    let archetype_iri = quad_map.keys()
        .find(|subject| match subject {
            Subject::NamedNode(nn) => nn == &ARCHETYPE_CLASS,
            _ => false,
        })
        .ok_or("No archetype found")?;
    
    // Get all quads for this archetype
    let archetype_quads = quad_map.get(archetype_iri).unwrap_or(&vec![]);
    
    // Extract required fields
    let title = extract_string_value(archetype_quads, &ARCHETYPE_TITLE)?;
    let status = extract_archetype_status(archetype_quads)?;
    let application_contract = extract_named_node_value(archetype_quads, &APPLICATION_CONTRACT)?;
    let infrastructure_contract_template = extract_named_node_value(archetype_quads, &INFRASTRUCTURE_CONTRACT_TEMPLATE)?;
    
    // Extract collections
    let infrastructure_contract_instances = extract_named_nodes(archetype_quads, &INFRASTRUCTURE_CONTRACT_INSTANCES);
    let application_task_types = extract_named_nodes(archetype_quads, &APPLICATION_TASK_TYPES);
    let infrastructure_task_types = extract_named_nodes(archetype_quads, &INFRASTRUCTURE_TASK_TYPES);
    let archetype_audits = extract_named_nodes(archetype_quads, &ARCHETYPE_AUDITS);
    let seam_audits = extract_named_nodes(archetype_quads, &SEAM_AUDITS);
    
    // Extract evidence
    let evidence = extract_archetype_evidence(archetype_quads)?;
    
    // Extract provenance
    let provenance = extract_provenance(archetype_quads)?;
    
    Ok(Archetype {
        id: archetype_iri.as_named_node().unwrap().clone(),
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
        provenance,
    })
}

fn extract_string_value(quads: &[Quad], predicate: &NamedNode) -> Result<String, String> {
    quads.iter()
        .find(|q| q.predicate == *predicate && q.object.is_literal())
        .map(|q| q.object.as_literal().unwrap().value().to_string())
        .ok_or_else(|| format!("Missing required string value for predicate {:?}", predicate))
}

fn extract_named_node_value(quads: &[Quad], predicate: &NamedNode) -> Result<NamedNode, String> {
    quads.iter()
        .find(|q| q.predicate == *predicate && q.object.is_named_node())
        .map(|q| q.object.as_named_node().unwrap().clone())
        .ok_or_else(|| format!("Missing required named node value for predicate {:?}", predicate))
}

fn extract_named_nodes(quads: &[Quad], predicate: &NamedNode) -> Vec<NamedNode> {
    quads.iter()
        .filter(|q| q.predicate == *predicate && q.object.is_named_node())
        .map(|q| q.object.as_named_node().unwrap().clone())
        .collect()
}

fn extract_archetype_status(quads: &[Quad]) -> Result<ArchetypeStatus, String> {
    let status_value = extract_named_node_value(quads, &ARCHETYPE_STATUS)?;
    
    match status_value.as_str() {
        "https://decision-archetype.org/ns/dec#candidate" => Ok(ArchetypeStatus::Candidate),
        "https://decision-archetype.org/ns/dec#standard" => Ok(ArchetypeStatus::Standard),
        "https://decision-archetype.org/ns/dec#quarantined" => Ok(ArchetypeStatus::Quarantined),
        _ => Err(format!("Invalid archetype status: {:?}", status_value)),
    }
}

fn extract_archetype_evidence(quads: &[Quad]) -> Result<ArchetypeEvidence, String> {
    // Find the evidence node
    let evidence_node = quads.iter()
        .find(|q| q.predicate == *ARCHETYPE_EVIDENCE && q.object.is_named_node())
        .map(|q| q.object.as_named_node().unwrap().clone())
        .ok_or("Missing archetype evidence")?;
        
    // Collect all evidence quads for this node
    let evidence_quads: Vec<Quad> = quads.iter()
        .filter(|q| q.subject == evidence_node)
        .cloned()
        .collect();
    
    let archetype_layer_estimate = extract_float_value(&evidence_quads, &ARCHETYPE_LAYER_ESTIMATE)?;
    let instance_variance = extract_variance(&evidence_quads)?;
    let application_contract_held_invariant = extract_boolean_value(&evidence_quads, &APPLICATION_CONTRACT_HELD_INVARIANT)?;
    let coverage_note = extract_string_value(&evidence_quads, &COVERAGE_NOTE)?;
    
    Ok(ArchetypeEvidence {
        archetype_layer_estimate,
        instance_variance,
        application_contract_held_invariant,
        coverage_note,
    })
}

fn extract_float_value(quads: &[Quad], predicate: &NamedNode) -> Result<f32, String> {
    quads.iter()
        .find(|q| q.predicate == *predicate && q.object.is_literal())
        .and_then(|q| q.object.as_literal().unwrap().value().parse::<f32>().ok())
        .ok_or_else(|| format!("Missing or invalid float value for predicate {:?}", predicate))
}

fn extract_boolean_value(quads: &[Quad], predicate: &NamedNode) -> Result<bool, String> {
    quads.iter()
        .find(|q| q.predicate == *predicate && q.object.is_literal())
        .and_then(|q| q.object.as_literal().unwrap().value().parse::<bool>().ok())
        .ok_or_else(|| format!("Missing or invalid boolean value for predicate {:?}", predicate))
}

fn extract_variance(quads: &[Quad]) -> Result<Variance, String> {
    let variance_value = extract_named_node_value(quads, &INSTANCE_VARIANCE)?;
    
    match variance_value.as_str() {
        "https://decision-archetype.org/ns/dec#low" => Ok(Variance::Low),
        "https://decision-archetype.org/ns/dec#medium" => Ok(Variance::Medium),
        "https://decision-archetype.org/ns/dec#high" => Ok(Variance::High),
        _ => Err(format!("Invalid instance variance: {:?}", variance_value)),
    }
}

fn extract_provenance(quads: &[Quad]) -> Result<Provenance, String> {
    // Find the provenance node
    let provenance_node = quads.iter()
        .find(|q| q.predicate == *PROVENANCE && q.object.is_named_node())
        .map(|q| q.object.as_named_node().unwrap().clone())
        .ok_or("Missing provenance")?;
        
    // Collect all provenance quads for this node
    let provenance_quads: Vec<Quad> = quads.iter()
        .filter(|q| q.subject == provenance_node)
        .cloned()
        .collect();
    
    let mechanical = extract_named_node_value(&provenance_quads, &MECHANICAL_PROVENANCE)?;
    let motivational = extract_named_node_value(&provenance_quads, &MOTIVATIONAL_PROVENANCE)?;
    
    Ok(Provenance {
        mechanical,
        motivational,
    })
}