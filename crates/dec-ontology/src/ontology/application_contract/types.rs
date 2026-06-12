//! In-memory shape of `dec:ApplicationContract` (FT-148 / ADR-082 §3).

use std::path::PathBuf;

use oxrdf::NamedNode;

use crate::ontology::provenance::Provenance;
use crate::vocab::IRI_DEC_APPLICATION_CONTRACT_PREFIX;

/// Build the canonical instance IRI for a contract id
/// (`…/ns/contract/app/<contract-id>`).
#[must_use]
pub fn application_contract_iri(contract_id: &str) -> NamedNode {
    NamedNode::new_unchecked(format!(
        "{IRI_DEC_APPLICATION_CONTRACT_PREFIX}{contract_id}"
    ))
}

/// The application half of an archetype's two parallel contracts
/// (ADR-082): the architectural conventions every application-family
/// TaskType conforms to.
#[derive(Debug, Clone, PartialEq)]
pub struct ApplicationContract {
    /// Instance IRI (`…/ns/contract/app/<contract-id>`).
    pub id: NamedNode,
    /// Back-reference to the owning Archetype (FT-147).
    pub archetype: NamedNode,
    /// Language/runtime convention (e.g. "C# / .NET 9").
    pub language_runtime: Convention,
    /// Layering rule (e.g. "Clean Architecture dependency rule").
    pub layering_rule: Convention,
    /// Feature organisation (e.g. "vertical slices").
    pub feature_organisation: Convention,
    /// Persistence model (e.g. "SQL domain model + EF Core conventions").
    pub persistence_model: Convention,
    /// Endpoint convention (endpoint == contract == frontend-call == test).
    pub endpoint_convention: Convention,
    /// Cross-cutting conventions (auth, validation, error handling, …).
    pub cross_cutting: Vec<Convention>,
    /// Mechanical + motivational provenance (FT-072 / FT-073).
    pub provenance: Provenance,
}

impl ApplicationContract {
    /// The six required conventions plus cross-cutting entries, in
    /// emission order — parser/emitter symmetry leans on this.
    #[must_use]
    pub fn conventions(&self) -> Vec<&Convention> {
        let mut all = vec![
            &self.language_runtime,
            &self.layering_rule,
            &self.feature_organisation,
            &self.persistence_model,
            &self.endpoint_convention,
        ];
        all.extend(self.cross_cutting.iter());
        all
    }
}

/// One checkable architectural rule inside a contract (ADR-082 §3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Convention {
    /// Convention IRI (a sub-resource of the contract).
    pub id: NamedNode,
    /// Short name (e.g. "slice", "clean-architecture", "persistence").
    pub name: String,
    /// Repo-relative path of the convention body
    /// (`forge/archetypes/{id}/application/conventions/{name}.md`).
    pub body_path: PathBuf,
    /// → ArchetypeAudit IRI that checks this convention (FT-152).
    pub audit_id: Option<NamedNode>,
    /// `false` → TaskTypes conforming to this convention are
    /// `not-safely-dispatchable` (propagated by FT-150/FT-153).
    pub checkable: bool,
}
