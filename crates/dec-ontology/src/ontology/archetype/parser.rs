//! Quad-iterator → struct parsing for `dec:Archetype` (FT-147).
//!
//! Symmetric with [`super::emitter`] — every field the emitter writes is
//! read here, witnessed by the round-trip tests.

use oxrdf::{NamedNode, Quad, Subject, Term};
use thiserror::Error;

use crate::ontology::provenance::Provenance;
use crate::ontology::MOTIVATIONAL_PREDICATES;
use crate::vocab::{
    IRI_DEC_APPLICATION_CONTRACT, IRI_DEC_APPLICATION_CONTRACT_HELD_INVARIANT,
    IRI_DEC_APPLICATION_TASK_TYPE, IRI_DEC_ARCHETYPE, IRI_DEC_ARCHETYPE_AUDIT,
    IRI_DEC_ARCHETYPE_LAYER_ESTIMATE, IRI_DEC_ARCHETYPE_STATUS, IRI_DEC_ARCHETYPE_TITLE,
    IRI_DEC_COVERAGE_NOTE, IRI_DEC_INFRASTRUCTURE_CONTRACT_INSTANCE,
    IRI_DEC_INFRASTRUCTURE_CONTRACT_TEMPLATE, IRI_DEC_INFRASTRUCTURE_TASK_TYPE,
    IRI_DEC_INSTANCE_VARIANCE, IRI_DEC_SEAM_AUDIT,
};

use super::types::{Archetype, ArchetypeEvidence, ArchetypeStatus, Variance};

const RDF_TYPE: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#type";

/// Parse failures for `dec:Archetype` quads.
#[derive(Debug, Error)]
pub enum ArchetypeParseError {
    /// No subject typed `dec:Archetype` in the quad set.
    #[error("no dec:Archetype subject in quad set")]
    NoArchetypeSubject,
    /// A required field is absent for the subject.
    #[error("archetype <{subject}> is missing required field {field}")]
    MissingField {
        /// Archetype subject IRI.
        subject: String,
        /// Vocabulary predicate of the absent field.
        field: &'static str,
    },
    /// A field carried a value outside its controlled vocabulary or datatype.
    #[error("archetype <{subject}> field {field} has invalid value {value:?}")]
    InvalidValue {
        /// Archetype subject IRI.
        subject: String,
        /// Vocabulary predicate of the offending field.
        field: &'static str,
        /// Offending lexical value.
        value: String,
    },
    /// The mechanical provenance block (FT-072) is incomplete.
    #[error("archetype <{subject}> is missing the mechanical provenance block (FT-072)")]
    MissingProvenance {
        /// Archetype subject IRI.
        subject: String,
    },
}

fn iri_object(q: &Quad) -> Option<NamedNode> {
    match &q.object {
        Term::NamedNode(n) => Some(n.clone()),
        _ => None,
    }
}

fn literal_object(q: &Quad) -> Option<String> {
    match &q.object {
        Term::Literal(l) => Some(l.value().to_string()),
        _ => None,
    }
}

/// Reassemble the first `dec:Archetype` subject found in `quads`.
pub fn quads_to_archetype(quads: &[Quad]) -> Result<Archetype, ArchetypeParseError> {
    let subject = quads
        .iter()
        .find(|q| {
            q.predicate.as_str() == RDF_TYPE
                && matches!(&q.object, Term::NamedNode(n) if n.as_str() == IRI_DEC_ARCHETYPE)
        })
        .and_then(|q| match &q.subject {
            Subject::NamedNode(n) => Some(n.clone()),
            _ => None,
        })
        .ok_or(ArchetypeParseError::NoArchetypeSubject)?;

    let mine: Vec<&Quad> = quads
        .iter()
        .filter(|q| q.subject == subject.clone().into())
        .collect();

    let missing = |field: &'static str| ArchetypeParseError::MissingField {
        subject: subject.as_str().to_string(),
        field,
    };
    let invalid = |field: &'static str, value: String| ArchetypeParseError::InvalidValue {
        subject: subject.as_str().to_string(),
        field,
        value,
    };

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

    for q in &mine {
        match q.predicate.as_str() {
            IRI_DEC_ARCHETYPE_TITLE => title = literal_object(q),
            IRI_DEC_ARCHETYPE_STATUS => {
                let raw = literal_object(q).ok_or_else(|| missing(IRI_DEC_ARCHETYPE_STATUS))?;
                status = Some(
                    ArchetypeStatus::parse(&raw)
                        .ok_or_else(|| invalid(IRI_DEC_ARCHETYPE_STATUS, raw))?,
                );
            }
            IRI_DEC_APPLICATION_CONTRACT => application_contract = iri_object(q),
            IRI_DEC_INFRASTRUCTURE_CONTRACT_TEMPLATE => {
                infrastructure_contract_template = iri_object(q);
            }
            IRI_DEC_INFRASTRUCTURE_CONTRACT_INSTANCE => {
                infrastructure_contract_instances.extend(iri_object(q));
            }
            IRI_DEC_APPLICATION_TASK_TYPE => application_task_types.extend(iri_object(q)),
            IRI_DEC_INFRASTRUCTURE_TASK_TYPE => infrastructure_task_types.extend(iri_object(q)),
            IRI_DEC_ARCHETYPE_AUDIT => archetype_audits.extend(iri_object(q)),
            IRI_DEC_SEAM_AUDIT => seam_audits.extend(iri_object(q)),
            IRI_DEC_ARCHETYPE_LAYER_ESTIMATE => {
                let raw =
                    literal_object(q).ok_or_else(|| missing(IRI_DEC_ARCHETYPE_LAYER_ESTIMATE))?;
                archetype_layer_estimate = Some(
                    raw.parse::<f32>()
                        .map_err(|_| invalid(IRI_DEC_ARCHETYPE_LAYER_ESTIMATE, raw))?,
                );
            }
            IRI_DEC_INSTANCE_VARIANCE => {
                let raw = literal_object(q).ok_or_else(|| missing(IRI_DEC_INSTANCE_VARIANCE))?;
                instance_variance = Some(
                    Variance::parse(&raw).ok_or_else(|| invalid(IRI_DEC_INSTANCE_VARIANCE, raw))?,
                );
            }
            IRI_DEC_APPLICATION_CONTRACT_HELD_INVARIANT => {
                let raw = literal_object(q)
                    .ok_or_else(|| missing(IRI_DEC_APPLICATION_CONTRACT_HELD_INVARIANT))?;
                application_contract_held_invariant = Some(
                    raw.parse::<bool>()
                        .map_err(|_| invalid(IRI_DEC_APPLICATION_CONTRACT_HELD_INVARIANT, raw))?,
                );
            }
            IRI_DEC_COVERAGE_NOTE => coverage_note = literal_object(q),
            _ => {}
        }
    }

    let provenance =
        Provenance::from_quads(quads, &subject, MOTIVATIONAL_PREDICATES).ok_or_else(|| {
            ArchetypeParseError::MissingProvenance {
                subject: subject.as_str().to_string(),
            }
        })?;

    Ok(Archetype {
        id: subject.clone(),
        title: title.ok_or_else(|| missing(IRI_DEC_ARCHETYPE_TITLE))?,
        status: status.ok_or_else(|| missing(IRI_DEC_ARCHETYPE_STATUS))?,
        application_contract: application_contract
            .ok_or_else(|| missing(IRI_DEC_APPLICATION_CONTRACT))?,
        infrastructure_contract_template: infrastructure_contract_template
            .ok_or_else(|| missing(IRI_DEC_INFRASTRUCTURE_CONTRACT_TEMPLATE))?,
        infrastructure_contract_instances,
        application_task_types,
        infrastructure_task_types,
        archetype_audits,
        seam_audits,
        evidence: ArchetypeEvidence {
            archetype_layer_estimate: archetype_layer_estimate
                .ok_or_else(|| missing(IRI_DEC_ARCHETYPE_LAYER_ESTIMATE))?,
            instance_variance: instance_variance
                .ok_or_else(|| missing(IRI_DEC_INSTANCE_VARIANCE))?,
            application_contract_held_invariant: application_contract_held_invariant
                .ok_or_else(|| missing(IRI_DEC_APPLICATION_CONTRACT_HELD_INVARIANT))?,
            coverage_note: coverage_note.ok_or_else(|| missing(IRI_DEC_COVERAGE_NOTE))?,
        },
        provenance,
    })
}
