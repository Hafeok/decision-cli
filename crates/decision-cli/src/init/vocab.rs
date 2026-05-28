//! IRI constants for the init pipeline.

pub(super) const RDF_TYPE: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#type";
pub(super) const RDF_FIRST: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#first";
pub(super) const RDF_REST: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#rest";
pub(super) const RDF_NIL: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#nil";

pub(super) const PROV_ACTIVITY: &str = "http://www.w3.org/ns/prov#Activity";
pub(super) const PROV_GENERATED_BY: &str = "http://www.w3.org/ns/prov#wasGeneratedBy";
pub(super) const PROV_DERIVED_FROM: &str = "http://www.w3.org/ns/prov#wasDerivedFrom";
pub(super) const PROV_AT_TIME: &str = "http://www.w3.org/ns/prov#atTime";

pub(super) const DEC_NAME: &str = "https://decision-cli.dev/ns#name";
pub(super) const DEC_TITLE: &str = "https://decision-cli.dev/ns#title";
pub(super) const DEC_DESCRIPTION: &str = "https://decision-cli.dev/ns#description";
pub(super) const DEC_TERMINAL_VALUE_ACTION: &str =
    "https://decision-cli.dev/ns#terminalValueAction";
pub(super) const DEC_AUTHORIZED_GOALS: &str = "https://decision-cli.dev/ns#authorizedGoals";
pub(super) const DEC_COMPATIBLE_GOALS: &str = "https://decision-cli.dev/ns#compatibleGoals";
pub(super) const DEC_DEFINITION_SOURCE: &str = "https://decision-cli.dev/ns#definitionSource";
pub(super) const DEC_DEFINITION_HASH: &str = "https://decision-cli.dev/ns#definitionHash";
pub(super) const DEC_ONTOLOGY_VERSION: &str = "https://decision-cli.dev/ns#ontologyVersion";
pub(super) const DEC_DEFINITION_FORM: &str = "https://decision-cli.dev/ns#definitionForm";
pub(super) const DEC_VALUE_STREAM_CLASS: &str = "https://decision-cli.dev/ns#ValueStream";
pub(super) const DEC_SESSION_CLASS: &str = "https://decision-cli.dev/ns#Session";
