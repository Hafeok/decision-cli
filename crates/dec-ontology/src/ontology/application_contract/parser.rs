//! Quad-iterator → struct parsing for `dec:ApplicationContract` (FT-148).
//!
//! Symmetric with [`super::emitter`]; Conventions parse inline from
//! their sub-resource quads.

use std::path::PathBuf;

use oxrdf::{NamedNode, Quad, Subject, Term};
use thiserror::Error;

use crate::ontology::provenance::Provenance;
use crate::ontology::MOTIVATIONAL_PREDICATES;
use crate::vocab::{
    IRI_DEC_APPLICATION_CONTRACT_CLASS, IRI_DEC_CONTRACT_ARCHETYPE, IRI_DEC_CONVENTION_AUDIT_ID,
    IRI_DEC_CONVENTION_BODY_PATH, IRI_DEC_CONVENTION_CHECKABLE, IRI_DEC_CONVENTION_NAME,
    IRI_DEC_CROSS_CUTTING, IRI_DEC_ENDPOINT_CONVENTION, IRI_DEC_FEATURE_ORGANISATION,
    IRI_DEC_LANGUAGE_RUNTIME, IRI_DEC_LAYERING_RULE, IRI_DEC_PERSISTENCE_MODEL,
};

use super::types::{ApplicationContract, Convention};

const RDF_TYPE: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#type";

/// Parse failures for `dec:ApplicationContract` quads.
#[derive(Debug, Error)]
pub enum ContractParseError {
    /// No subject typed `dec:ApplicationContract` in the quad set.
    #[error("no dec:ApplicationContract subject in quad set")]
    NoContractSubject,
    /// A required field is absent for the subject.
    #[error("contract <{subject}> is missing required field {field}")]
    MissingField {
        /// Contract (or convention) subject IRI.
        subject: String,
        /// Vocabulary predicate of the absent field.
        field: &'static str,
    },
    /// A convention sub-resource is malformed.
    #[error("convention <{subject}> is malformed: {detail}")]
    MalformedConvention {
        /// Convention subject IRI.
        subject: String,
        /// What is missing or invalid.
        detail: String,
    },
    /// The mechanical provenance block (FT-072) is incomplete.
    #[error("contract <{subject}> is missing the mechanical provenance block (FT-072)")]
    MissingProvenance {
        /// Contract subject IRI.
        subject: String,
    },
}

fn iri_object(quads: &[Quad], subject: &NamedNode, predicate: &str) -> Option<NamedNode> {
    quads.iter().find_map(|q| {
        if q.subject == subject.clone().into() && q.predicate.as_str() == predicate {
            match &q.object {
                Term::NamedNode(n) => Some(n.clone()),
                _ => None,
            }
        } else {
            None
        }
    })
}

fn literal_object(quads: &[Quad], subject: &NamedNode, predicate: &str) -> Option<String> {
    quads.iter().find_map(|q| {
        if q.subject == subject.clone().into() && q.predicate.as_str() == predicate {
            match &q.object {
                Term::Literal(l) => Some(l.value().to_string()),
                _ => None,
            }
        } else {
            None
        }
    })
}

fn parse_convention(quads: &[Quad], id: &NamedNode) -> Result<Convention, ContractParseError> {
    let malformed = |detail: &str| ContractParseError::MalformedConvention {
        subject: id.as_str().to_string(),
        detail: detail.to_string(),
    };
    let name =
        literal_object(quads, id, IRI_DEC_CONVENTION_NAME).ok_or_else(|| malformed("no name"))?;
    let body_path = literal_object(quads, id, IRI_DEC_CONVENTION_BODY_PATH)
        .ok_or_else(|| malformed("no body_path"))?;
    let checkable = literal_object(quads, id, IRI_DEC_CONVENTION_CHECKABLE)
        .ok_or_else(|| malformed("no checkable flag"))?
        .parse::<bool>()
        .map_err(|_| malformed("checkable is not a boolean"))?;
    Ok(Convention {
        id: id.clone(),
        name,
        body_path: PathBuf::from(body_path),
        audit_id: iri_object(quads, id, IRI_DEC_CONVENTION_AUDIT_ID),
        checkable,
    })
}

/// Reassemble the first `dec:ApplicationContract` subject found in `quads`.
pub fn quads_to_application_contract(
    quads: &[Quad],
) -> Result<ApplicationContract, ContractParseError> {
    let subject = quads
        .iter()
        .find(|q| {
            q.predicate.as_str() == RDF_TYPE
                && matches!(&q.object, Term::NamedNode(n) if n.as_str() == IRI_DEC_APPLICATION_CONTRACT_CLASS)
        })
        .and_then(|q| match &q.subject {
            Subject::NamedNode(n) => Some(n.clone()),
            _ => None,
        })
        .ok_or(ContractParseError::NoContractSubject)?;

    let missing = |field: &'static str| ContractParseError::MissingField {
        subject: subject.as_str().to_string(),
        field,
    };

    let required = |predicate: &'static str| -> Result<Convention, ContractParseError> {
        let id = iri_object(quads, &subject, predicate).ok_or_else(|| missing(predicate))?;
        parse_convention(quads, &id)
    };

    let cross_cutting = quads
        .iter()
        .filter(|q| {
            q.subject == subject.clone().into() && q.predicate.as_str() == IRI_DEC_CROSS_CUTTING
        })
        .filter_map(|q| match &q.object {
            Term::NamedNode(n) => Some(n.clone()),
            _ => None,
        })
        .map(|id| parse_convention(quads, &id))
        .collect::<Result<Vec<_>, _>>()?;

    let provenance = Provenance::from_quads(quads, &subject, MOTIVATIONAL_PREDICATES).ok_or(
        ContractParseError::MissingProvenance {
            subject: subject.as_str().to_string(),
        },
    )?;

    Ok(ApplicationContract {
        archetype: iri_object(quads, &subject, IRI_DEC_CONTRACT_ARCHETYPE)
            .ok_or_else(|| missing(IRI_DEC_CONTRACT_ARCHETYPE))?,
        language_runtime: required(IRI_DEC_LANGUAGE_RUNTIME)?,
        layering_rule: required(IRI_DEC_LAYERING_RULE)?,
        feature_organisation: required(IRI_DEC_FEATURE_ORGANISATION)?,
        persistence_model: required(IRI_DEC_PERSISTENCE_MODEL)?,
        endpoint_convention: required(IRI_DEC_ENDPOINT_CONVENTION)?,
        cross_cutting,
        provenance,
        id: subject,
    })
}
