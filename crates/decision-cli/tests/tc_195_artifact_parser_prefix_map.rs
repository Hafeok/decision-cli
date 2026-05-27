//! TC-195 — `ArtifactRef::parse` maps known prefixes to the right
//! `ArtifactKind` and rejects malformed input with `ParseError` whose
//! message mentions a supported prefix.
//!
//! Validates: FT-110.

use decision_cli::core::drive::artifact::{ArtifactKind, ArtifactRef, ParseError};

#[test]
fn tc_195_artifact_parser_prefix_map() {
    let happy = [
        ("FT-019", ArtifactKind::Feature),
        ("TC-027", ArtifactKind::TestCriterion),
        ("VG-100", ArtifactKind::VerificationGraph),
        ("ENV-002", ArtifactKind::Environment),
        ("ENV-001-ephemeral-cli", ArtifactKind::Environment),
        ("ADR-066", ArtifactKind::Adr),
    ];
    for (s, expected) in happy {
        let r = ArtifactRef::parse(s).unwrap_or_else(|e| panic!("{s} should parse: {e}"));
        assert_eq!(r.kind, expected, "kind for {s}");
        assert_eq!(r.short_id, s, "short_id preserved for {s}");
    }

    let bad = [
        ("", "Empty"),
        ("FT019", "UnknownPrefix"),
        ("XX-000", "UnknownPrefix"),
        ("feature-019", "UnknownPrefix"),
        ("FT-", "Malformed"),
        ("FT-abc", "Malformed"),
    ];
    for (s, expected_variant) in bad {
        let err = ArtifactRef::parse(s).expect_err(s);
        let tag = match &err {
            ParseError::Empty => "Empty",
            ParseError::UnknownPrefix { .. } => "UnknownPrefix",
            ParseError::Malformed { .. } => "Malformed",
        };
        assert_eq!(tag, expected_variant, "wrong variant for {s:?}: {err}");
        let msg = format!("{err}");
        // Every error must surface at least one supported prefix so the
        // operator can self-correct.
        assert!(
            msg.contains("FT-") || msg.contains("expected a prefix"),
            "msg should hint at supported prefixes: {msg}"
        );
    }
}
