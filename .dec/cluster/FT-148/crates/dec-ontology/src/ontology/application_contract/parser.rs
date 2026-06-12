use crate::ontology::application_contract::{ApplicationContract, Convention};
use crate::ontology::provenance::{MotivationalEdge, Provenance};
use crate::vocab;
use oxrdf::{NamedNode, NamedNodeRef, Quad, Subject, GraphName};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum ApplicationContractParseError {
    /// No subject typed `dec:ApplicationContract` in the quad set.
    #[error("no dec:ApplicationContract subject in quad set")]
    NoApplicationContractSubject,
    /// A required field is absent for the subject.
    #[error("application contract <{subject}> is missing required field {field}")]
    MissingField {
        /// Application contract subject IRI.
        subject: String,
        /// Vocabulary predicate of the absent field.
        field: &'static str,
    },
    /// A field carried a value outside its controlled vocabulary or datatype.
    #[error("application contract <{subject}> field {field} has invalid value {value:?}")]
    InvalidValue {
        /// Application contract subject IRI.
        subject: String,
        /// Vocabulary predicate of the offending field.
        field: &'static str,
        /// Offending lexical value.
        value: String,
    },
    /// The mechanical provenance block (FT-072) is incomplete.
    #[error("application contract <{subject}> is missing the mechanical provenance block (FT-072)")]
    MissingProvenance {
        /// Application contract subject IRI.
        subject: String,
    },
}

/// Parse a list of RDF quads into a single application contract.
///
/// This function assumes the quads contain exactly one `dec:ApplicationContract`.
pub fn quads_to_application_contract(quads: &[Quad]) -> Result<ApplicationContract, ApplicationContractParseError> {
    // Find the subject typed as dec:ApplicationContract
    let subject = quads
        .iter()
        .find(|quad| quad.predicate == vocab::TYPE && quad.object == vocab::APPLICATION_CONTRACT)
        .map(|quad| quad.subject.clone())
        .ok_or(ApplicationContractParseError::NoApplicationContractSubject)?;

    // Collect all quads related to this subject
    let subject_quads: Vec<&Quad> = quads.iter().filter(|q| q.subject == subject).collect();

    // Extract the archetype
    let archetype = subject_quads
        .iter()
        .find(|q| q.predicate == vocab::ARCHETYPE)
        .map(|q| q.object.as_named_node().ok_or_else(|| ApplicationContractParseError::InvalidValue {
            subject: subject.to_string(),
            field: "archetype",
            value: q.object.to_string(),
        }))
        .transpose()?
        .ok_or_else(|| ApplicationContractParseError::MissingField {
            subject: subject.to_string(),
            field: "archetype",
        })?;

    // Extract language runtime
    let language_runtime = extract_convention(&subject_quads, &subject, vocab::LANGUAGE_RUNTIME)?;

    // Extract layering rule
    let layering_rule = extract_convention(&subject_quads, &subject, vocab::LAYERING_RULE)?;

    // Extract feature organisation
    let feature_organisation = extract_convention(&subject_quads, &subject, vocab::FEATURE_ORGANISATION)?;

    // Extract persistence model
    let persistence_model = extract_convention(&subject_quads, &subject, vocab::PERSISTENCE_MODEL)?;

    // Extract endpoint convention
    let endpoint_convention = extract_convention(&subject_quads, &subject, vocab::ENDPOINT_CONVENTION)?;

    // Extract cross-cutting concerns
    let cross_cutting = extract_conventions(&subject_quads, &subject, vocab::CROSS_CUTTING)?;

    // Extract provenance
    let provenance = extract_provenance(&subject_quads, &subject)?;

    Ok(ApplicationContract {
        id: subject.as_named_node().unwrap().clone(),
        archetype: archetype.clone(),
        language_runtime,
        layering_rule,
        feature_organisation,
        persistence_model,
        endpoint_convention,
        cross_cutting,
        provenance,
    })
}

fn extract_convention(
    subject_quads: &[&Quad],
    subject: &Subject,
    predicate: NamedNodeRef<'_>,
) -> Result<Convention, ApplicationContractParseError> {
    let convention_quad = subject_quads
        .iter()
        .find(|q| q.predicate == predicate)
        .ok_or_else(|| ApplicationContractParseError::MissingField {
            subject: subject.to_string(),
            field: predicate.as_str(),
        })?;

    let convention_subject = convention_quad.object.as_named_node().ok_or_else(|| ApplicationContractParseError::InvalidValue {
        subject: subject.to_string(),
        field: predicate.as_str(),
        value: convention_quad.object.to_string(),
    })?;

    // Collect all quads for this convention
    let convention_quads: Vec<&Quad> = subject_quads.iter().filter(|q| q.subject == *convention_subject).collect();
    Convention::from_quads(&convention_quads, convention_subject)
        .map_err(|e| ApplicationContractParseError::InvalidValue {
            subject: subject.to_string(),
            field: predicate.as_str(),
            value: e.to_string(),
        })
}

fn extract_conventions(
    subject_quads: &[&Quad],
    subject: &Subject,
    predicate: NamedNodeRef<'_>,
) -> Result<Vec<Convention>, ApplicationContractParseError> {
    let mut conventions = Vec::new();

    for quad in subject_quads.iter().filter(|q| q.predicate == predicate) {
        let convention_subject = quad.object.as_named_node().ok_or_else(|| ApplicationContractParseError::InvalidValue {
            subject: subject.to_string(),
            field: predicate.as_str(),
            value: quad.object.to_string(),
        })?;

        // Collect all quads for this convention
        let convention_quads: Vec<&Quad> = subject_quads.iter().filter(|q| q.subject == *convention_subject).collect();
        let convention = Convention::from_quads(&convention_quads, convention_subject)
            .map_err(|e| ApplicationContractParseError::InvalidValue {
                subject: subject.to_string(),
                field: predicate.as_str(),
                value: e.to_string(),
            })?;
        conventions.push(convention);
    }

    Ok(conventions)
}

fn extract_provenance(
    subject_quads: &[&Quad],
    subject: &Subject,
) -> Result<Provenance, ApplicationContractParseError> {
    // Find the provenance block by looking for the presence of a prov:wasGeneratedBy triple
    let generated_by = subject_quads
        .iter()
        .find(|q| q.predicate == vocab::WAS_GENERATED_BY)
        .map(|q| q.object.as_named_node().ok_or_else(|| ApplicationContractParseError::InvalidValue {
            subject: subject.to_string(),
            field: "prov:wasGeneratedBy",
            value: q.object.to_string(),
        }))
        .transpose()?
        .ok_or_else(|| ApplicationContractParseError::MissingProvenance {
            subject: subject.to_string(),
        })?;

    let attributed_to = subject_quads
        .iter()
        .find(|q| q.predicate == vocab::WAS_ATTRIBUTED_TO)
        .map(|q| q.object.as_named_node().ok_or_else(|| ApplicationContractParseError::InvalidValue {
            subject: subject.to_string(),
            field: "prov:wasAttributedTo",
            value: q.object.to_string(),
        }))
        .transpose()?
        .ok_or_else(|| ApplicationContractParseError::MissingProvenance {
            subject: subject.to_string(),
        })?;

    let generated_at_time = subject_quads
        .iter()
        .find(|q| q.predicate == vocab::GENERATED_AT_TIME)
        .map(|q| q.object.as_literal().and_then(|l| l.value().map(|v| v.to_string())))
        .flatten()
        .ok_or_else(|| ApplicationContractParseError::MissingProvenance {
            subject: subject.to_string(),
        })?;

    // Extract motivational edges
    let mut motivational = Vec::new();
    for quad in subject_quads.iter().filter(|q| q.predicate == vocab::MOTIVATIONAL_EDGE) {
        if let Some(target) = quad.object.as_named_node() {
            motivational.push(MotivationalEdge {
                predicate: quad.predicate.clone(),
                target: target.clone(),
            });
        }
    }

    Ok(Provenance {
        was_generated_by: generated_by.clone(),
        was_attributed_to: attributed_to.clone(),
        generated_at_time,
        motivational,
    })
}