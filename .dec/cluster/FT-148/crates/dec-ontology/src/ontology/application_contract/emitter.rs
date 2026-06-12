use crate::ontology::application_contract::ApplicationContract;
use crate::ontology::provenance::Provenance;
use crate::vocab;
use oxrdf::{GraphName, NamedNode, NamedNodeRef, Quad, Subject};

impl ApplicationContract {
    pub fn to_quads(&self, graph: NamedNodeRef<'_>) -> Vec<Quad> {
        let mut quads = vec![];
        let subject = self.id.clone().into();

        // Add type
        quads.push(Quad::new(
            subject.clone(),
            vocab::A,
            vocab::APPLICATION_CONTRACT.into(),
            graph,
        ));

        // Add archetype
        quads.push(Quad::new(
            subject.clone(),
            vocab::ARCHETYPE,
            self.archetype.clone().into(),
            graph,
        ));

        // Add language runtime
        quads.push(Quad::new(
            subject.clone(),
            vocab::LANGUAGE_RUNTIME,
            self.language_runtime.id.clone().into(),
            graph,
        ));

        // Add layering rule
        quads.push(Quad::new(
            subject.clone(),
            vocab::LAYERING_RULE,
            self.layering_rule.id.clone().into(),
            graph,
        ));

        // Add feature organisation
        quads.push(Quad::new(
            subject.clone(),
            vocab::FEATURE_ORGANISATION,
            self.feature_organisation.id.clone().into(),
            graph,
        ));

        // Add persistence model
        quads.push(Quad::new(
            subject.clone(),
            vocab::PERSISTENCE_MODEL,
            self.persistence_model.id.clone().into(),
            graph,
        ));

        // Add endpoint convention
        quads.push(Quad::new(
            subject.clone(),
            vocab::ENDPOINT_CONVENTION,
            self.endpoint_convention.id.clone().into(),
            graph,
        ));

        // Add cross-cutting conventions
        for convention in &self.cross_cutting {
            quads.push(Quad::new(
                subject.clone(),
                vocab::CROSS_CUTTING,
                convention.id.clone().into(),
                graph,
            ));
        }

        // Add provenance
        quads.extend(self.provenance.to_quads(&subject, graph));

        quads
    }
}

impl Provenance {
    pub fn to_quads(&self, subject: &Subject, graph: &GraphName) -> Vec<Quad> {
        let mut quads = vec![];

        quads.push(Quad::new(
            subject.clone(),
            vocab::WAS_GENERATED_BY,
            self.was_generated_by.clone().into(),
            graph,
        ));

        quads.push(Quad::new(
            subject.clone(),
            vocab::WAS_ATTRIBUTED_TO,
            self.was_attributed_to.clone().into(),
            graph,
        ));

        quads.push(Quad::new(
            subject.clone(),
            vocab::GENERATED_AT_TIME,
            self.generated_at_time.clone().into(),
            graph,
        ));

        for edge in &self.motivational {
            quads.push(Quad::new(
                subject.clone(),
                edge.predicate.clone().into(),
                edge.target.clone().into(),
                graph,
            ));
        }

        quads
    }
}