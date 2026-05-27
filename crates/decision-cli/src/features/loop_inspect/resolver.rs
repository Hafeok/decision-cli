//! Short-id resolution for the loop reporter (FT-109 / TC-194).
//!
//! Turns the PROV-O IRIs the orchestration store carries into the
//! operator-facing short ids the reporter renders. Anything that
//! doesn't match a known pattern falls back to the raw IRI.

/// Resolve a `source_session` IRI to a short label.
///
/// Patterns:
/// - `https://decision-cli.dev/ns/activity/verify-graph-run/VG-NNN/...` → `VG-NNN`
/// - `https://decision-cli.dev/ns/activity/verify-graph-generate/VG-NNN` → `verify-graph-author@VG-NNN`
/// - `https://decision-cli.dev/ns/activity/implement/<dispatch-id>` → `implement@<short>`
/// - else → raw IRI.
#[must_use]
pub fn short_for_session(iri: &str) -> String {
    if let Some(rest) = iri.strip_prefix("https://decision-cli.dev/ns/activity/verify-graph-run/") {
        if let Some((vg, _tail)) = rest.split_once('/') {
            return vg.to_string();
        }
        return rest.to_string();
    }
    if let Some(rest) = iri.strip_prefix("https://decision-cli.dev/ns/activity/verify-graph-generate/") {
        return format!("verify-graph-author@{rest}");
    }
    if let Some(rest) = iri.strip_prefix("https://decision-cli.dev/ns/activity/implement/") {
        let short: String = rest.chars().take(12).collect();
        return format!("implement@{short}");
    }
    iri.to_string()
}

/// Resolve an `addressing_artifact` IRI (a graph or a CodeChange).
///
/// Patterns:
/// - `https://decision-cli.dev/ns/graph/VG-NNN[-suffix]` → `VG-NNN[-suffix]`
/// - `https://decision-cli.dev/ns/code-change/CC-NNN` → `CC-NNN`
/// - `urn:dec:code-change:<uuid>` → `cc:<first8>`
/// - else → raw IRI.
#[must_use]
pub fn short_for_artifact(iri: &str) -> String {
    if let Some(rest) = iri.strip_prefix("https://decision-cli.dev/ns/graph/") {
        return rest.to_string();
    }
    if let Some(rest) = iri.strip_prefix("https://decision-cli.dev/ns/code-change/") {
        return rest.to_string();
    }
    if let Some(rest) = iri.strip_prefix("urn:dec:code-change:") {
        let short: String = rest.chars().take(8).collect();
        return format!("cc:{short}");
    }
    iri.to_string()
}

/// Resolve a feedback IRI to a label suitable for column display.
/// Always shows the URN's first 8 hex chars (after the `urn:dec:feedback:` prefix).
#[must_use]
pub fn short_for_feedback(iri: &str) -> String {
    if let Some(rest) = iri.strip_prefix("urn:dec:feedback:") {
        let short: String = rest.chars().take(8).collect();
        return format!("fb:{short}");
    }
    iri.to_string()
}

/// Resolve a TC IRI back to its short id (`TC-NNN`). Returns the
/// supplied string unchanged when the prefix doesn't match.
#[must_use]
pub fn short_for_tc(iri: &str) -> String {
    iri.strip_prefix("https://decision-cli.dev/ns/tc/")
        .unwrap_or(iri)
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verify_run_session_resolves_to_vg() {
        let iri = "https://decision-cli.dev/ns/activity/verify-graph-run/VG-007/ts-1234";
        assert_eq!(short_for_session(iri), "VG-007");
    }

    #[test]
    fn vga_dispatch_session_resolves_with_prefix() {
        let iri = "https://decision-cli.dev/ns/activity/verify-graph-generate/VG-098";
        assert_eq!(short_for_session(iri), "verify-graph-author@VG-098");
    }

    #[test]
    fn graph_artifact_resolves_to_vg() {
        assert_eq!(
            short_for_artifact("https://decision-cli.dev/ns/graph/VG-NEW-1"),
            "VG-NEW-1"
        );
    }

    #[test]
    fn code_change_artifact_resolves() {
        assert_eq!(
            short_for_artifact("https://decision-cli.dev/ns/code-change/CC-FIX-2"),
            "CC-FIX-2"
        );
    }

    #[test]
    fn unknown_iri_falls_back_to_raw() {
        let raw = "http://example.org/something-else";
        assert_eq!(short_for_session(raw), raw);
        assert_eq!(short_for_artifact(raw), raw);
    }
}
