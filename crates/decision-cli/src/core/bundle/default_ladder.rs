//! Pure default-stakes ladder per ADR-035 §"Who sets it" (FT-056).
//!
//! The ladder reads the focal artifact's class IRI plus (for feature_specs)
//! the count of linked cross-cutting ADRs. The function is intentionally
//! pure: callers (typically `core::bundle::assemble_for_role`) feed in a
//! [`FocalContext`] computed from the bundle composer's already-loaded
//! reads — no additional graph traversal happens inside this module.

use oxigraph::model::NamedNode;

use crate::core::vocab::{IRI_DEC_CAPABILITY, IRI_DEC_ROLE_BINDING};

use super::types::Stakes;

/// ADR scope per ADR-014. A feature_spec's `Elevated` stake fires when it
/// links to ≥ 2 [`AdrScope::CrossCutting`] ADRs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdrScope {
    /// Feature-specific ADR.
    FeatureSpecific,
    /// Cross-cutting ADR (governs every feature implicitly per ADR-014).
    CrossCutting,
}

/// Context the bundle composer hands to [`default_stakes_for`].
///
/// This is intentionally a value type so the function stays pure:
/// `default_stakes_for(FocalContext) -> Stakes` does no graph reads.
#[derive(Debug, Clone)]
pub struct FocalContext {
    /// Class IRI of the focal artifact (e.g. `dec:Capability`,
    /// `dec:RoleBinding`, `decproduct:FeatureSpec`, …).
    pub class_iri: NamedNode,
    /// True when the focal artifact represents an ontology change
    /// (predicate add, class add, shape add). Surfaced separately because
    /// not every ontology change has a single dedicated class.
    pub is_ontology_change: bool,
    /// True when the focal artifact defines a new artifact type
    /// (an `rdfs:Class` declaration that did not previously exist).
    pub is_new_artifact_type: bool,
    /// True when the focal artifact is itself a cross-cutting ADR (the
    /// `scope: cross-cutting` ADRs per ADR-014).
    pub is_cross_cutting_adr: bool,
    /// For feature_spec focal artifacts: scopes of the ADRs the spec
    /// links to. Empty for non-feature_spec focal artifacts.
    pub linked_adr_scopes: Vec<AdrScope>,
}

impl FocalContext {
    /// Open a context with just the class IRI; all judgement flags default
    /// to `false` and the linked-ADR list is empty.
    #[must_use]
    pub fn new(class_iri: NamedNode) -> Self {
        Self {
            class_iri,
            is_ontology_change: false,
            is_new_artifact_type: false,
            is_cross_cutting_adr: false,
            linked_adr_scopes: Vec::new(),
        }
    }

    /// Convenience: mark the focal artifact as an ontology change.
    #[must_use]
    pub fn ontology_change(mut self) -> Self {
        self.is_ontology_change = true;
        self
    }

    /// Convenience: mark the focal artifact as a new artifact-type definition.
    #[must_use]
    pub fn new_artifact_type(mut self) -> Self {
        self.is_new_artifact_type = true;
        self
    }

    /// Convenience: mark the focal artifact as a cross-cutting ADR.
    #[must_use]
    pub fn cross_cutting_adr(mut self) -> Self {
        self.is_cross_cutting_adr = true;
        self
    }

    /// Convenience: extend the linked ADR scope list (for feature_specs).
    #[must_use]
    pub fn with_adr_scope(mut self, scope: AdrScope) -> Self {
        self.linked_adr_scopes.push(scope);
        self
    }

    fn count_cross_cutting_adrs(&self) -> usize {
        self.linked_adr_scopes
            .iter()
            .filter(|s| matches!(s, AdrScope::CrossCutting))
            .count()
    }
}

/// Apply the ADR-035 §"Who sets it" ladder to the focal context.
///
/// Rules:
///
/// - Focal is `dec:Capability`, `dec:RoleBinding`, an ontology change, or a
///   new artifact type definition → `Foundational`.
/// - Focal is a cross-cutting ADR or a feature_spec linked to ≥ 2
///   cross-cutting ADRs → `Elevated`.
/// - Otherwise → `Routine`.
///
/// The function is total and pure — every input yields exactly one
/// [`Stakes`] value; no failure mode (unrecognised inputs fall through to
/// `Routine`, the conservative default per FT-056 §Error handling).
#[must_use]
pub fn default_stakes_for(ctx: &FocalContext) -> Stakes {
    if is_foundational(ctx) {
        return Stakes::Foundational;
    }
    if is_elevated(ctx) {
        return Stakes::Elevated;
    }
    Stakes::Routine
}

fn is_foundational(ctx: &FocalContext) -> bool {
    if ctx.is_ontology_change || ctx.is_new_artifact_type {
        return true;
    }
    let iri = ctx.class_iri.as_str();
    iri == IRI_DEC_CAPABILITY || iri == IRI_DEC_ROLE_BINDING
}

fn is_elevated(ctx: &FocalContext) -> bool {
    if ctx.is_cross_cutting_adr {
        return true;
    }
    ctx.count_cross_cutting_adrs() >= 2
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cls(iri: &str) -> NamedNode {
        NamedNode::new(iri).expect("class iri")
    }

    #[test]
    fn capability_focal_is_foundational() {
        let ctx = FocalContext::new(cls("https://decision-cli.dev/ns#Capability"));
        assert_eq!(default_stakes_for(&ctx), Stakes::Foundational);
    }

    #[test]
    fn role_binding_focal_is_foundational() {
        let ctx = FocalContext::new(cls("https://decision-cli.dev/ns#RoleBinding"));
        assert_eq!(default_stakes_for(&ctx), Stakes::Foundational);
    }

    #[test]
    fn ontology_change_is_foundational() {
        let ctx = FocalContext::new(cls("https://example.com/anything")).ontology_change();
        assert_eq!(default_stakes_for(&ctx), Stakes::Foundational);
    }

    #[test]
    fn new_artifact_type_is_foundational() {
        let ctx = FocalContext::new(cls("https://example.com/anything")).new_artifact_type();
        assert_eq!(default_stakes_for(&ctx), Stakes::Foundational);
    }

    #[test]
    fn cross_cutting_adr_is_elevated() {
        let ctx = FocalContext::new(cls("https://example.com/adr/014")).cross_cutting_adr();
        assert_eq!(default_stakes_for(&ctx), Stakes::Elevated);
    }

    #[test]
    fn feature_spec_with_two_cross_cutting_adrs_is_elevated() {
        let ctx = FocalContext::new(cls("https://example.com/featurespec"))
            .with_adr_scope(AdrScope::CrossCutting)
            .with_adr_scope(AdrScope::CrossCutting)
            .with_adr_scope(AdrScope::FeatureSpecific);
        assert_eq!(default_stakes_for(&ctx), Stakes::Elevated);
    }

    #[test]
    fn feature_spec_with_one_cross_cutting_adr_is_routine() {
        let ctx = FocalContext::new(cls("https://example.com/featurespec"))
            .with_adr_scope(AdrScope::CrossCutting)
            .with_adr_scope(AdrScope::FeatureSpecific);
        assert_eq!(default_stakes_for(&ctx), Stakes::Routine);
    }

    #[test]
    fn feature_spec_with_zero_cross_cutting_adrs_is_routine() {
        let ctx = FocalContext::new(cls("https://example.com/featurespec"))
            .with_adr_scope(AdrScope::FeatureSpecific)
            .with_adr_scope(AdrScope::FeatureSpecific);
        assert_eq!(default_stakes_for(&ctx), Stakes::Routine);
    }

    #[test]
    fn unrecognised_class_falls_through_to_routine() {
        let ctx = FocalContext::new(cls("https://example.com/unknown"));
        assert_eq!(default_stakes_for(&ctx), Stakes::Routine);
    }

    #[test]
    fn ladder_is_deterministic() {
        // Same input → same output (the invariant `default_stakes_for is pure`).
        let ctx = FocalContext::new(cls("https://decision-cli.dev/ns#Capability"));
        let a = default_stakes_for(&ctx);
        let b = default_stakes_for(&ctx);
        let c = default_stakes_for(&ctx);
        assert_eq!(a, b);
        assert_eq!(b, c);
    }
}
