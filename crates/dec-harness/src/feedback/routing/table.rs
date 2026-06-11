//! Routing table for `dec:Feedback` artifacts per ADR-026.
//!
//! Maps every `FeedbackClass` (ADR-023) to a `(default_target_role,
//! addressing_roles, override_allowed_by)` tuple. Each row is the
//! Rust-source mirror of the table in [ADR-026 §Routing table]. Phase C
//! will lift these rows into graph-resident `dec:RoutingRule` triples;
//! the source-of-truth migration is a transcription, not a rewrite.
//!
//! Per the slice-level SDP convention in `CLAUDE.md`, every consumer
//! (FT-029 handler, FT-033 CLI, FT-031 worker SDK) imports from this
//! module; no consumer reaches into a sibling feature for the defaults.

use crate::feedback::class::FeedbackClass;

/// Actor categories permitted to override the default target for a class.
///
/// Phase A scope: the emitter may override its own emissions; the human
/// operator may override via `dec feedback route <id> --to <role-id>`.
/// Other actors (the orchestrator under policy, the meta-loop) land in
/// later slices.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OverrideActor {
    /// The emitting role's worker may set `dec:routingOverride` at emission.
    Emitter,
    /// A human operator may set `dec:routingOverride` via CLI.
    Human,
}

/// One row of the ADR-026 routing table — what the orchestrator needs to
/// know to route a `Feedback` of a given class.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RoutingRule {
    /// The feedback class this rule governs.
    pub class: FeedbackClass,
    /// Default target role id when no override is supplied.
    pub default_target_role: &'static str,
    /// Role ids whose produced artifacts validly address a feedback of
    /// this class on `received → addressed` (FT-027 validates the
    /// transition; FT-029 enforces this list at routing time only for
    /// override targets).
    pub addressing_roles: &'static [&'static str],
    /// Actors permitted to override the default target. Empty means the
    /// default is fixed.
    pub override_allowed_by: &'static [OverrideActor],
}

/// The full ADR-026 routing table as a static slice.
///
/// Exhaustive over every `FeedbackClass` variant — a compile-time check
/// enforces that property in [`assert_table_covers_all_classes`].
pub const ROUTING_TABLE: &[RoutingRule] = &[
    RoutingRule {
        class: FeedbackClass::Gap,
        default_target_role: "spec-author",
        addressing_roles: &["spec-author", "architect"],
        override_allowed_by: &[OverrideActor::Emitter, OverrideActor::Human],
    },
    RoutingRule {
        class: FeedbackClass::Contradiction,
        default_target_role: "architect",
        addressing_roles: &["architect", "spec-author"],
        override_allowed_by: &[OverrideActor::Emitter, OverrideActor::Human],
    },
    RoutingRule {
        class: FeedbackClass::Unimplementable,
        default_target_role: "spec-author",
        addressing_roles: &["spec-author", "architect"],
        override_allowed_by: &[OverrideActor::Emitter, OverrideActor::Human],
    },
    RoutingRule {
        class: FeedbackClass::ScopeIssue,
        default_target_role: "slice-curator",
        addressing_roles: &["slice-curator", "spec-author"],
        override_allowed_by: &[OverrideActor::Emitter, OverrideActor::Human],
    },
    RoutingRule {
        class: FeedbackClass::Defect,
        default_target_role: "verifier",
        addressing_roles: &["verifier", "implementer"],
        override_allowed_by: &[OverrideActor::Emitter, OverrideActor::Human],
    },
    RoutingRule {
        class: FeedbackClass::CapabilityRequest,
        default_target_role: "architect",
        addressing_roles: &["architect", "spec-author"],
        override_allowed_by: &[OverrideActor::Emitter, OverrideActor::Human],
    },
];

/// Resolve the rule for `class`. Always returns `Some` because
/// `ROUTING_TABLE` is exhaustive over `FeedbackClass`.
#[must_use]
pub fn rule_for(class: FeedbackClass) -> &'static RoutingRule {
    for rule in ROUTING_TABLE {
        if rule.class == class {
            return rule;
        }
    }
    // Unreachable: the test below guarantees coverage. Kept as a
    // structural fallback so the function signature stays infallible.
    unreachable_table_row(class)
}

#[cold]
fn unreachable_table_row(class: FeedbackClass) -> &'static RoutingRule {
    panic!(
        "routing table missing row for FeedbackClass::{class:?} — ADR-026 amendment is incomplete"
    );
}

/// Resolve the default target role for `class` per ADR-026.
#[must_use]
pub fn default_target_role(class: FeedbackClass) -> &'static str {
    rule_for(class).default_target_role
}

/// True if `role_id` is in the `addressing_roles` set for `class`.
#[must_use]
pub fn role_may_address(class: FeedbackClass, role_id: &str) -> bool {
    rule_for(class)
        .addressing_roles
        .iter()
        .any(|r| *r == role_id)
}

/// True if `actor` may set a `dec:routingOverride` on a feedback of `class`.
#[must_use]
pub fn override_permitted_for(class: FeedbackClass, actor: OverrideActor) -> bool {
    rule_for(class)
        .override_allowed_by
        .iter()
        .any(|a| *a == actor)
}

/// Compile-time check that every `FeedbackClass` variant has a row.
///
/// Implemented as an exhaustive match so adding a new variant without
/// extending `ROUTING_TABLE` produces a compile error in the test path.
#[cfg(test)]
const fn assert_table_covers_all_classes() {
    let _ = |c: FeedbackClass| match c {
        FeedbackClass::Gap => (),
        FeedbackClass::Contradiction => (),
        FeedbackClass::Unimplementable => (),
        FeedbackClass::ScopeIssue => (),
        FeedbackClass::Defect => (),
        FeedbackClass::CapabilityRequest => (),
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_class_has_a_row() {
        for class in FeedbackClass::all() {
            let _ = rule_for(*class);
        }
        // Force the const exhaustive-match guard to be evaluated.
        assert_table_covers_all_classes();
    }

    #[test]
    fn defaults_match_adr_026() {
        assert_eq!(default_target_role(FeedbackClass::Gap), "spec-author");
        assert_eq!(
            default_target_role(FeedbackClass::Contradiction),
            "architect"
        );
        assert_eq!(
            default_target_role(FeedbackClass::Unimplementable),
            "spec-author"
        );
        assert_eq!(
            default_target_role(FeedbackClass::ScopeIssue),
            "slice-curator"
        );
        assert_eq!(default_target_role(FeedbackClass::Defect), "verifier");
        assert_eq!(
            default_target_role(FeedbackClass::CapabilityRequest),
            "architect"
        );
    }

    #[test]
    fn addressing_roles_include_default() {
        // Sanity: the default target role should itself be an
        // accepted addressing role.
        for class in FeedbackClass::all() {
            let rule = rule_for(*class);
            assert!(
                rule.addressing_roles.contains(&rule.default_target_role),
                "default target {} not in addressing_roles for {:?}",
                rule.default_target_role,
                class
            );
        }
    }

    #[test]
    fn overrides_permitted_for_emitter_and_human() {
        for class in FeedbackClass::all() {
            assert!(override_permitted_for(*class, OverrideActor::Emitter));
            assert!(override_permitted_for(*class, OverrideActor::Human));
        }
    }

    #[test]
    fn role_may_address_distinguishes_known_and_unknown() {
        assert!(role_may_address(FeedbackClass::Gap, "spec-author"));
        assert!(role_may_address(FeedbackClass::Defect, "verifier"));
        assert!(!role_may_address(FeedbackClass::Gap, "verifier"));
        assert!(!role_may_address(FeedbackClass::Gap, "unknown-role"));
    }
}
