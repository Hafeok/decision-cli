//! Tool-name validation for `dec_<noun>_<verb>` (ADR-029).
//!
//! ADR-029 mandates every MCP tool name follow `dec_<noun>_<verb>`,
//! e.g. `dec_verify_env_new`. The registry enforces the rule at
//! registration so a malformed descriptor fails startup rather than
//! reaching the MCP wire. The check is performed by [`validate_tool_name`]
//! and returns a structured [`NamingError`] when the name is rejected.

use thiserror::Error;

/// Reasons a tool name is rejected. Each variant carries the original
/// name so the diagnostic on stderr names the offender directly.
#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum NamingError {
    /// The name is the empty string.
    #[error("tool name is empty")]
    Empty,

    /// The name does not start with `dec_`.
    #[error("tool name '{0}' must start with 'dec_'")]
    MissingPrefix(String),

    /// The name contains characters outside the allowed set
    /// (`[a-z0-9_]`, with `_` as the only separator).
    #[error(
        "tool name '{0}' must match ^dec_[a-z0-9]+(_[a-z0-9]+)*$ \
         (lowercase letters/digits + '_' separators, no spaces)"
    )]
    InvalidCharacters(String),

    /// The name has the prefix but no noun/verb after it.
    #[error("tool name '{0}' must have at least one segment after 'dec_'")]
    MissingSegments(String),
}

/// Validate `name` against the ADR-029 `dec_<noun>_<verb>` convention.
///
/// The accepted shape is `dec` followed by one or more `_<segment>`
/// groups where each `<segment>` is `[a-z0-9]+`. Examples:
///
/// * `dec_health_check` — accepted (one noun, one verb).
/// * `dec_verify_env_new` — accepted (noun-noun-verb is permitted).
/// * `verify_env_new` — rejected, missing `dec_` prefix.
/// * `dec_` — rejected, no segment after the prefix.
/// * `dec_Foo_bar` — rejected, uppercase.
/// * `dec_foo bar` — rejected, contains a space.
pub fn validate_tool_name(name: &str) -> Result<(), NamingError> {
    if name.is_empty() {
        return Err(NamingError::Empty);
    }
    let Some(tail) = name.strip_prefix("dec_") else {
        return Err(NamingError::MissingPrefix(name.to_string()));
    };
    if tail.is_empty() {
        return Err(NamingError::MissingSegments(name.to_string()));
    }
    for segment in tail.split('_') {
        if segment.is_empty() {
            return Err(NamingError::InvalidCharacters(name.to_string()));
        }
        if !segment
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        {
            return Err(NamingError::InvalidCharacters(name.to_string()));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_canonical_names() {
        validate_tool_name("dec_mcp_ping").expect("two-segment name");
        validate_tool_name("dec_verify_env_new").expect("three-segment name");
        validate_tool_name("dec_health_check").expect("simple noun_verb");
        validate_tool_name("dec_feature1_list").expect("digit in segment");
    }

    #[test]
    fn rejects_empty() {
        assert!(matches!(validate_tool_name(""), Err(NamingError::Empty)));
    }

    #[test]
    fn rejects_missing_prefix() {
        assert!(matches!(
            validate_tool_name("verify_env_new"),
            Err(NamingError::MissingPrefix(_))
        ));
        assert!(matches!(
            validate_tool_name("product_feature_new"),
            Err(NamingError::MissingPrefix(_))
        ));
    }

    #[test]
    fn rejects_space_in_name() {
        // TC-051 AC #1: the explicit "bad name" example.
        assert!(matches!(
            validate_tool_name("bad name"),
            Err(NamingError::MissingPrefix(_))
        ));
        assert!(matches!(
            validate_tool_name("dec_bad name"),
            Err(NamingError::InvalidCharacters(_))
        ));
    }

    #[test]
    fn rejects_uppercase() {
        assert!(matches!(
            validate_tool_name("dec_Verify_env_new"),
            Err(NamingError::InvalidCharacters(_))
        ));
    }

    #[test]
    fn rejects_empty_segment() {
        assert!(matches!(
            validate_tool_name("dec_"),
            Err(NamingError::MissingSegments(_))
        ));
        assert!(matches!(
            validate_tool_name("dec_foo__bar"),
            Err(NamingError::InvalidCharacters(_))
        ));
        assert!(matches!(
            validate_tool_name("dec_foo_"),
            Err(NamingError::InvalidCharacters(_))
        ));
    }

    #[test]
    fn rejects_dashes_and_dots() {
        assert!(matches!(
            validate_tool_name("dec_verify-env-new"),
            Err(NamingError::InvalidCharacters(_))
        ));
        assert!(matches!(
            validate_tool_name("dec.verify.env"),
            Err(NamingError::MissingPrefix(_))
        ));
    }
}
