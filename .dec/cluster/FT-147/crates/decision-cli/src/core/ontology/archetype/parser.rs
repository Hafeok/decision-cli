use crate::core::ontology::archetype::{Archetype, ArchetypeStatus, ArchetypeEvidence, Variance, Provenance, ProvenanceMechanical, ProvenanceMotivational};
use crate::core::ontology::vocab::archetype as vocab;
use crate::core::ontology::vocab::prov as prov_vocab;
use crate::core::ontology::vocab::motivational as motivation_vocab;
use crate::core::ontology::vocab::task_type as task_type_vocab;
use crate::core::ontology::vocab::contract as contract_vocab;
use crate::core::ontology::vocab::audit as audit_vocab;
use crate::core::ontology::vocab::evidence as evidence_vocab;
use crate::core::ontology::vocab::provenance as provenance_vocab;
use oxigraph::model::{NamedNode, Quad, Subject, GraphName, IriRef};
use std::collections::HashMap;
use std::convert::TryInto;

pub fn parse_archetype(quads: &[Quad]) -> Result<Archetype, String> {
    let mut quads_by_subject = HashMap::<Subject, Vec<&Quad>>::new();
    for quad in quads {
        quads_by_subject.entry(quad.subject.clone()).or_default().push(quad);
    }

    let subject = quads.first()
        .ok_or("No quads provided")?
        .subject
        .clone();

    let id = subject.try_into().map_err(|_| "Invalid subject for archetype")?;

    let mut title = None;
    let mut status = None;
    let mut application_contract = None;
    let mut infrastructure_contract_template = None;
    let mut infrastructure_contract_instances = Vec::new();
    let mut application_task_types = Vec::new();
    let mut infrastructure_task_types = Vec::new();
    let mut archetype_audits = Vec::new();
    let mut seam_audits = Vec::new();
    let mut archetype_layer_estimate = None;
    let mut instance_variance = None;
    let mut application_contract_held_invariant = None;
    let mut coverage_note = None;
    let mut mechanical_provenance = None;
    let mut motivational_provenance = None;

    for quad in &quads_by_subject[&subject] {
        if quad.predicate == vocab::ARCHETYPE_TITLE.into() {
            if let Some(object) = quad.object.as_literal() {
                title = Some(object.value().to_string());
            }
        } else if quad.predicate == vocab::ARCHETYPE_STATUS.into() {
            if let Some(object) = quad.object.as_literal() {
                let status_str = object.value();
                status = match status_str {
                    "candidate" => Some(ArchetypeStatus::Candidate),
                    "standard" => Some(ArchetypeStatus::Standard),
                    "quarantined" => Some(ArchetypeStatus::Quarantined),
                    _ => return Err(format!("Invalid archetype status: {}", status_str)),
                };
            }
        } else if quad.predicate == vocab::APPLICATION_CONTRACT.into() {
            if let Some(object) = quad.object.as_named_node() {
                application_contract = Some(object.clone());
            }
        } else if quad.predicate == vocab::INFRASTRUCTURE_CONTRACT_TEMPLATE.into() {
            if let Some(object) = quad.object.as_named_node() {
                infrastructure_contract_template = Some(object.clone());
            }
        } else if quad.predicate == vocab::INFRASTRUCTURE_CONTRACT_INSTANCES.into() {
            if let Some(object) = quad.object.as_named_node() {
                infrastructure_contract_instances.push(object.clone());
            }
        } else if quad.predicate == vocab::APPLICATION_TASK_TYPES.into() {
            if let Some(object) = quad.object.as_named_node() {
                application_task_types.push(object.clone());
            }
        } else if quad.predicate == vocab::INFRASTRUCTURE_TASK_TYPES.into() {
            if let Some(object) = quad.object.as_named_node() {
                infrastructure_task_types.push(object.clone());
            }
        } else if quad.predicate == vocab::ARCHETYPE_AUDITS.into() {
            if let Some(object) = quad.object.as_named_node() {
                archetype_audits.push(object.clone());
            }
        } else if quad.predicate == vocab::SEAM_AUDITS.into() {
            if let Some(object) = quad.object.as_named_node() {
                seam_audits.push(object.clone());
            }
        } else if quad.predicate == vocab::ARCHETYPE_LAYER_ESTIMATE.into() {
            if let Some(object) = quad.object.as_literal() {
                if let Ok(value) = object.value().parse::<f32>() {
                    archetype_layer_estimate = Some(value);
                }
            }
        } else if quad.predicate == vocab::INSTANCE_VARIANCE.into() {
            if let Some(object) = quad.object.as_literal() {
                let variance_str = object.value();
                instance_variance = match variance_str {
                    "low" => Some(Variance::Low),
                    "medium" => Some(Variance::Medium),
                    "high" => Some(Variance::High),
                    _ => return Err(format!("Invalid instance variance: {}", variance_str)),
                };
            }
        } else if quad.predicate == vocab::APPLICATION_CONTRACT_HELD_INVARIANT.into() {
            if let Some(object) = quad.object.as_literal() {
                if let Ok(value) = object.value().parse::<bool>() {
                    application_contract_held_invariant = Some(value);
                }
            }
        } else if quad.predicate == vocab::COVERAGE_NOTE.into() {
            if let Some(object) = quad.object.as_literal() {
                coverage_note = Some(object.value().to_string());
            }
        } else if quad.predicate == provenance_vocab::MECHANICAL_PROVENANCE.into() {
            // Handle mechanical provenance
            let prov_subject = quad.subject.clone();
            let mut prov_quads = Vec::new();
            for q in quads_by_subject.get(&prov_subject).unwrap_or(&Vec::new()) {
                prov_quads.push(q.clone());
            }
            mechanical_provenance = Some(parse_mechanical_provenance(&prov_quads)?);
        } else if quad.predicate == provenance_vocab::MOTIVATIONAL_PROVENANCE.into() {
            // Handle motivational provenance
            let prov_subject = quad.subject.clone();
            let mut prov_quads = Vec::new();
            for q in quads_by_subject.get(&prov_subject).unwrap_or(&Vec::new()) {
                prov_quads.push(q.clone());
            }
            motivational_provenance = Some(parse_motivational_provenance(&prov_quads)?);
        }
    }

    // Check required fields
    let title = title.ok_or("Missing archetype title")?;
    let status = status.ok_or("Missing archetype status")?;
    let application_contract = application_contract.ok_or("Missing application contract")?;
    let infrastructure_contract_template = infrastructure_contract_template.ok_or("Missing infrastructure contract template")?;
    let archetype_layer_estimate = archetype_layer_estimate.ok_or("Missing archetype layer estimate")?;
    let instance_variance = instance_variance.ok_or("Missing instance variance")?;
    let application_contract_held_invariant = application_contract_held_invariant.unwrap_or(false);
    let coverage_note = coverage_note.unwrap_or_default();

    let evidence = ArchetypeEvidence {
        archetype_layer_estimate,
        instance_variance,
        application_contract_held_invariant,
        coverage_note,
    };

    let provenance = Provenance {
        mechanical: mechanical_provenance,
        motivational: motivational_provenance,
    };

    Ok(Archetype::new(
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
        provenance,
    ))
}

fn parse_mechanical_provenance(quads: &[&Quad]) -> Result<ProvenanceMechanical, String> {
    let mut generated_by = None;
    let mut generated_at = None;
    let mut generated_via = None;

    for quad in quads {
        if quad.predicate == prov_vocab::WAS_GENERATED_BY.into() {
            if let Some(object) = quad.object.as_named_node() {
                generated_by = Some(object.clone());
            }
        } else if quad.predicate == prov_vocab::GENERATED_AT.into() {
            if let Some(object) = quad.object.as_literal() {
                generated_at = Some(object.value().to_string());
            }
        } else if quad.predicate == prov_vocab::GENERATED_VIA.into() {
            if let Some(object) = quad.object.as_named_node() {
                generated_via = Some(object.clone());
            }
        }
    }

    Ok(ProvenanceMechanical {
        generated_by: generated_by.ok_or("Missing generated by")?,
        generated_at: generated_at.ok_or("Missing generated at")?,
        generated_via: generated_via.ok_or("Missing generated via")?,
    })
}

fn parse_motivational_provenance(quads: &[&Quad]) -> Result<ProvenanceMotivational, String> {
    let mut motivated_by = None;
    let mut motivated_via = None;

    for quad in quads {
        if quad.predicate == motivation_vocab::MOTIVATED_BY.into() {
            if let Some(object) = quad.object.as_named_node() {
                motivated_by = Some(object.clone());
            }
        } else if quad.predicate == motivation_vocab::MOTIVATED_VIA.into() {
            if let Some(object) = quad.object.as_named_node() {
                motivated_via = Some(object.clone());
            }
        }
    }

    Ok(ProvenanceMotivational {
        motivated_by: motivated_by.ok_or("Missing motivated by")?,
        motivated_via: motivated_via.ok_or("Missing motivated via")?,
    })
}