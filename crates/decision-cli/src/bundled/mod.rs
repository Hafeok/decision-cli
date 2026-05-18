//! Bundled definition library (FT-007 minimum surface needed by FT-006/FT-008).
//!
//! Slice 1 ships exactly one canonical `ValueAction`
//! (`va:shipped-feature`) and one `ValueStream` template
//! (`engineering-development`). Both are embedded via `include_str!`
//! (ADR-007). The resolver here is intentionally minimal — FT-007
//! will extend it with a richer catalog and lookup helpers.

/// Canonical bundled ValueAction (`va:shipped-feature`).
pub const SHIPPED_FEATURE_TTL: &str =
    include_str!("assets/value-actions/shipped-feature.ttl");

/// Bundled stream template (`engineering-development`).
pub const ENGINEERING_DEVELOPMENT_TTL: &str =
    include_str!("assets/streams/engineering-development.ttl");

/// IRI of the bundled `shipped-feature` ValueAction.
pub const SHIPPED_FEATURE_IRI: &str = "https://decision-cli.dev/ns/value-actions/shipped-feature";

/// IRI of the bundled `engineering-development` stream template.
pub const ENGINEERING_DEVELOPMENT_IRI: &str =
    "https://decision-cli.dev/ns/streams/engineering-development";

/// A bundled ValueAction definition document.
pub struct BundledValueAction {
    /// Stable URI for the action (e.g. `va:shipped-feature`).
    pub iri: &'static str,
    /// Raw Turtle bytes — embedded.
    pub ttl: &'static str,
}

/// A bundled ValueStream template document.
pub struct BundledStreamTemplate {
    /// Human-friendly name accepted by `dec init --template <name>`.
    pub name: &'static str,
    /// Stable URI for the stream artifact.
    pub iri: &'static str,
    /// Raw Turtle bytes — embedded.
    pub ttl: &'static str,
}

/// All bundled ValueActions known to the binary.
pub const VALUE_ACTIONS: &[BundledValueAction] = &[BundledValueAction {
    iri: SHIPPED_FEATURE_IRI,
    ttl: SHIPPED_FEATURE_TTL,
}];

/// All bundled stream templates known to the binary.
pub const STREAM_TEMPLATES: &[BundledStreamTemplate] = &[BundledStreamTemplate {
    name: "engineering-development",
    iri: ENGINEERING_DEVELOPMENT_IRI,
    ttl: ENGINEERING_DEVELOPMENT_TTL,
}];

/// Look up a bundled ValueAction by IRI.
#[must_use]
pub fn lookup_value_action(iri: &str) -> Option<&'static BundledValueAction> {
    VALUE_ACTIONS.iter().find(|va| va.iri == iri)
}

/// Look up a bundled stream template by `--template <name>`.
#[must_use]
pub fn lookup_stream_template(name: &str) -> Option<&'static BundledStreamTemplate> {
    STREAM_TEMPLATES.iter().find(|t| t.name == name)
}

/// Names of all bundled stream templates (for `--help` and errors).
#[must_use]
pub fn known_template_names() -> Vec<&'static str> {
    STREAM_TEMPLATES.iter().map(|t| t.name).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shipped_feature_is_bundled() {
        let va = lookup_value_action(SHIPPED_FEATURE_IRI).expect("bundled");
        assert!(va.ttl.contains("dec:compatibleGoals"));
    }

    #[test]
    fn engineering_development_template_is_bundled() {
        let t = lookup_stream_template("engineering-development").expect("bundled");
        assert_eq!(t.iri, ENGINEERING_DEVELOPMENT_IRI);
        assert!(t.ttl.contains("dec:authorizedGoals"));
    }

    #[test]
    fn unknown_template_is_none() {
        assert!(lookup_stream_template("does-not-exist").is_none());
    }

    #[test]
    fn unknown_value_action_is_none() {
        assert!(lookup_value_action("https://example.org/not-bundled").is_none());
    }
}
