use crate::ontology::provenance::{Provenance, MotivationalEdge};
use crate::vocab;
use oxrdf::{NamedNode, NamedNodeRef, Quad, GraphName, Subject, Term};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// An ApplicationContract describes the architectural conventions that govern a specific
/// application archetype. It is composed of a set of required conventions (language runtime,
/// layering rule, feature organisation, persistence model, endpoint convention) and a list
/// of cross-cutting concerns.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ApplicationContract {
    /// The IRI identifying this application contract.
    pub id: NamedNode,
    /// The archetype this application contract belongs to.
    pub archetype: NamedNode,
    /// The language runtime convention used in this application.
    pub language_runtime: Convention,
    /// The layering rule convention used in this application.
    pub layering_rule: Convention,
    /// The feature organisation convention used in this application.
    pub feature_organisation: Convention,
    /// The persistence model convention used in this application.
    pub persistence_model: Convention,
    /// The endpoint convention used in this application.
    pub endpoint_convention: Convention,
    /// Cross-cutting concerns that apply to this application.
    pub cross_cutting: Vec<Convention>,
    /// Provenance information for this application contract.
    pub provenance: Provenance,
}

/// A Convention represents a specific architectural rule or practice.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Convention {
    /// The IRI identifying this convention.
    pub id: NamedNode,
    /// The human-readable name of this convention.
    pub name: String,
    /// The filesystem path to the documentation for this convention.
    pub body_path: PathBuf,
    /// Optional IRI pointing to the audit that validates this convention.
    pub audit_id: Option<NamedNode>,
    /// Indicates whether this convention is checkable and thus contributes to dispatchability.
    pub checkable: bool,
}

impl ApplicationContract {
    /// Converts this ApplicationContract into RDF quads.
    pub fn to_quads(&self, graph: NamedNodeRef<'_>) -> Vec<Quad> {
        let mut quads = Vec::new();
        let subject = self.id.as_ref();

        // Add type
        quads.push(Quad::new(
            subject,
            vocab::DEC_TYPE_PREDICATE,
            vocab::APPLICATION_CONTRACT_CLASS.into(),
            graph,
        ));

        // Add archetype reference
        quads.push(Quad::new(
            subject,
            vocab::ARCHETYPE_PREDICATE,
            self.archetype.as_ref(),
            graph,
        ));

        // Add language runtime
        quads.extend(self.language_runtime.to_quads(subject, graph));

        // Add layering rule
        quads.extend(self.layering_rule.to_quads(subject, graph));

        // Add feature organisation
        quads.extend(self.feature_organisation.to_quads(subject, graph));

        // Add persistence model
        quads.extend(self.persistence_model.to_quads(subject, graph));

        // Add endpoint convention
        quads.extend(self.endpoint_convention.to_quads(subject, graph));

        // Add cross-cutting concerns
        for (index, convention) in self.cross_cutting.iter().enumerate() {
            let convention_subject = convention.id.as_ref();
            quads.push(Quad::new(
                subject,
                vocab::CROSS_CUTTING_PREDICATE,
                convention_subject,
                graph,
            ));
            quads.extend(convention.to_quads(convention_subject, graph));
        }

        // Add provenance
        quads.extend(self.provenance.to_quads(subject, graph));

        quads
    }
}

impl Convention {
    /// Converts this Convention into RDF quads.
    pub fn to_quads(&self, parent_subject: &Subject, graph: NamedNodeRef<'_>) -> Vec<Quad> {
        let mut quads = Vec::new();
        let subject = self.id.as_ref();

        // Add type
        quads.push(Quad::new(
            subject,
            vocab::DEC_TYPE_PREDICATE,
            vocab::CONVENTION_CLASS.into(),
            graph,
        ));

        // Add name
        quads.push(Quad::new(
            subject,
            vocab::CONVENTION_NAME,
            self.name.as_str().into(),
            graph,
        ));

        // Add body path
        quads.push(Quad::new(
            subject,
            vocab::CONVENTION_BODY_PATH,
            self.body_path.to_string_lossy().as_ref().into(),
            graph,
        ));

        // Add audit ID if present
        if let Some(audit_id) = &self.audit_id {
            quads.push(Quad::new(
                subject,
                vocab::CONVENTION_AUDIT_ID,
                audit_id.as_ref(),
                graph,
            ));
        }

        // Add checkable flag
        quads.push(Quad::new(
            subject,
            vocab::CONVENTION_CHECKABLE,
            self.checkable.to_string().into(),
            graph,
        ));

        quads
    }

    /// Creates a new Convention from RDF quads.
    pub fn from_quads(quads: &[Quad], convention_subject: &Subject) -> Result<Self, Box<dyn std::error::Error>> {
        let mut name: Option<String> = None;
        let mut body_path: Option<PathBuf> = None;
        let mut audit_id: Option<NamedNode> = None;
        let mut checkable: Option<bool> = None;

        for quad in quads {
            if quad.subject() != convention_subject {
                continue;
            }

            match quad.predicate().as_str() {
                s if s == vocab::CONVENTION_NAME => {
                    if let Some(literal) = quad.object().as_literal() {
                        name = Some(literal.value().to_string());
                    }
                }
                s if s == vocab::CONVENTION_BODY_PATH => {
                    if let Some(literal) = quad.object().as_literal() {
                        body_path = Some(PathBuf::from(literal.value()));
                    }
                }
                s if s == vocab::CONVENTION_AUDIT_ID => {
                    if let Some(node) = quad.object().as_named_node() {
                        audit_id = Some(node.into());
                    }
                }
                s if s == vocab::CONVENTION_CHECKABLE => {
                    if let Some(literal) = quad.object().as_literal() {
                        if let Ok(checkable_bool) = literal.value().parse::<bool>() {
                            checkable = Some(checkable_bool);
                        }
                    }
                }
                _ => {}
            }
        }

        Ok(Self {
            id: convention_subject.as_named_node().unwrap().into(),
            name: name.ok_or("Missing convention name")?,
            body_path: body_path.ok_or("Missing convention body path")?,
            audit_id,
            checkable: checkable.ok_or("Missing convention checkable flag")?,
        })
    }
}