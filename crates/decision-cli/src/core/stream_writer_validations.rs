//! SHACL-validate-by-type helpers for [`super::stream_writer::StreamWriter`].

use anyhow::{anyhow, Result};
use oxigraph::model::Quad;

use crate::core::bundle::validate_quads as validate_bundle_quads;
use crate::core::feedback::validate_quads as validate_feedback_quads;
use crate::core::ontology::capability::validate_quads as validate_capability_quads;
use crate::core::ontology::catalog::validate_quads_with_store as validate_catalog_quads_with_store;
use crate::core::ontology::coverage_waiver::validate_quads as validate_waiver_quads;
use crate::core::ontology::role_binding::validate_quads as validate_role_binding_quads;
use crate::core::ontology::verdict::validate_quads as validate_verdict_quads;
use crate::core::ontology::verification_env::validate_quads as validate_env_quads;
use crate::core::ontology::verification_graph::validate_quads as validate_graph_quads;
use crate::core::ontology::verification_result::validate_quads_with_store as validate_result_quads_with_store;
use oxigraph::store::Store;

/// SHACL-validate every `dec:VerificationVerdict` subject present in
/// `quads` (FT-020 / ADR-018). The error message keeps the
/// `SHACL violation` prefix used by sibling validators so existing
/// callers that match on the prefix continue to work uniformly.
pub(super) fn validate_verdicts(quads: &[Quad]) -> Result<()> {
    validate_verdict_quads(quads).map_err(|err| {
        anyhow!(
            "SHACL violation: verification verdict mutation refused\n{}",
            err.report
        )
    })
}

/// SHACL-validate every `dec:Feedback` subject present in `quads`
/// (FT-026 / ADR-022).
pub(super) fn validate_feedback(quads: &[Quad]) -> Result<()> {
    validate_feedback_quads(quads)
        .map_err(|err| anyhow!("SHACL violation: feedback mutation refused\n{}", err.report))
}

/// SHACL-validate every `dec:VerificationEnvironment` subject present in
/// `quads` (FT-035 / ADR-028).
pub(super) fn validate_envs(quads: &[Quad]) -> Result<()> {
    validate_env_quads(quads).map_err(|err| {
        anyhow!(
            "SHACL violation: verification environment mutation refused\n{}",
            err.report
        )
    })
}

/// SHACL-validate every `dec:VerificationGraph` / `dec:VerificationStep`
/// subject present in `quads` (FT-036 / ADR-028).
pub(super) fn validate_graphs(quads: &[Quad]) -> Result<()> {
    validate_graph_quads(quads).map_err(|err| {
        anyhow!(
            "SHACL violation: verification graph mutation refused\n{}",
            err.report
        )
    })
}

/// SHACL-validate every `dec:VerificationGraphResult` /
/// `dec:VerificationStepTrace` subject present in `quads` (FT-097 / ADR-028).
pub(super) fn validate_results(quads: &[Quad], store: Option<&Store>) -> Result<()> {
    validate_result_quads_with_store(quads, store).map_err(|err| {
        anyhow!(
            "SHACL violation: verification result mutation refused\n{}",
            err.report
        )
    })
}

/// SHACL-validate every `dec:CoverageWaiver` subject present in `quads`
/// (FT-047 / ADR-031).
pub(super) fn validate_waivers(quads: &[Quad]) -> Result<()> {
    validate_waiver_quads(quads).map_err(|err| {
        anyhow!(
            "SHACL violation: coverage waiver mutation refused\n{}",
            err.report
        )
    })
}

/// SHACL-validate every `dec:Capability` subject present in `quads`
/// (FT-054 / ADR-033).
pub(super) fn validate_capabilities(quads: &[Quad]) -> Result<()> {
    validate_capability_quads(quads).map_err(|err| {
        anyhow!(
            "SHACL violation: capability mutation refused\n{}",
            err.report
        )
    })
}

/// SHACL-validate every `dec:RoleBinding` / `dec:EscalationStep` /
/// `dec:EscalationTrigger` subject present in `quads` (FT-055 /
/// ADR-033 / ADR-034).
pub(super) fn validate_role_bindings(quads: &[Quad]) -> Result<()> {
    validate_role_binding_quads(quads).map_err(|err| {
        anyhow!(
            "SHACL violation: role binding mutation refused\n{}",
            err.report
        )
    })
}

/// SHACL-validate every `dec:Bundle` subject present in `quads`
/// (FT-056 / ADR-035).
pub(super) fn validate_bundles(quads: &[Quad]) -> Result<()> {
    validate_bundle_quads(quads)
        .map_err(|err| anyhow!("SHACL violation: bundle mutation refused\n{}", err.report))
}

/// SHACL-validate every `dec:CapabilityReference` / `dec:OntologyDescription`
/// / `dec:ExemplarGraph` subject present in `quads` (FT-101 / ADR-066).
/// The validator consults `store` for cross-mutation invariants: command
/// uniqueness across the active set, the single-active OntologyDescription
/// rule, and ExemplarGraph backing-result resolution.
pub(super) fn validate_catalog(quads: &[Quad], store: Option<&Store>) -> Result<()> {
    validate_catalog_quads_with_store(quads, store).map_err(|err| {
        anyhow!("SHACL violation: catalog mutation refused\n{}", err.report)
    })
}
