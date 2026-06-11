//! SPARQL fragments backing [`super::agreement`] (FT-024 §Outputs).
//!
//! The query joins `dec:DispatchGroup` → action `Session` →
//! interpretation `Session` → `dec:VerificationVerdict`, projecting the
//! columns the Rust counter consumes. Window + role filtering is
//! interpolated into the query body — SPARQL has no portable numeric /
//! string bind for our oxigraph version, and the inputs are validated
//! upstream by [`super::agreement::agreement`] before they reach the
//! formatter so injection is structurally impossible.

use dec_ontology::vocab::{
    DISPATCH_STATUS_ACTION_FAILED, DISPATCH_STATUS_AWAITING_AMENDMENT, DISPATCH_STATUS_COMPLETE,
    DISPATCH_STATUS_INTERPRETATION_FAILED, DISPATCH_STATUS_INTERPRETATION_REJECTED,
};

/// The set of `dec:dispatchStatus` literals that mark a DispatchGroup as
/// having reached a terminal lifecycle state (mirrors
/// [`crate::dispatch::DispatchStatus::is_terminal`]).
#[must_use]
pub const fn terminal_statuses() -> [&'static str; 5] {
    [
        DISPATCH_STATUS_COMPLETE,
        DISPATCH_STATUS_INTERPRETATION_REJECTED,
        DISPATCH_STATUS_AWAITING_AMENDMENT,
        DISPATCH_STATUS_ACTION_FAILED,
        DISPATCH_STATUS_INTERPRETATION_FAILED,
    ]
}

/// Build the agreement SPARQL query for the given window + role filter.
///
/// Returned columns:
/// - `?group` — the DispatchGroup IRI (DISTINCT).
/// - `?status` — `dec:dispatchStatus` literal.
/// - `?actionStatus` — `dec:status` literal on the action session (may
///   be unbound when the action is still in flight, in which case the
///   group will not be in a terminal status either).
/// - `?actionRole` — `dec:role` literal on the action session.
/// - `?verdictValue` — `dec:verdict` literal on the verifier verdict,
///   unbound for groups that never reached interpretation.
/// - `?actionStartedAt` — `prov:atTime` on the action session, used for
///   window filtering. Rendered as `xsd:dateTime` literal.
#[must_use]
pub fn build_query(window: Option<(&str, &str)>, role_filter: Option<&str>) -> String {
    let role = role_clause(role_filter);
    let win = window_clause(window);
    let default_branch = group_pattern();
    let named_branch = group_pattern();
    format!(
        "PREFIX dec:  <https://decision-cli.dev/ns#> \
         PREFIX prov: <http://www.w3.org/ns/prov#> \
         PREFIX xsd:  <http://www.w3.org/2001/XMLSchema#> \
         SELECT DISTINCT ?group ?status ?actionStatus ?actionRole ?verdictValue ?actionStartedAt \
         WHERE {{ \
           {{ {default_branch} }} UNION {{ GRAPH ?g {{ {named_branch} }} }} \
           {role} \
           {win} \
         }}"
    )
}

/// One arm of the UNION'd group pattern. Same body in default and
/// named-graph variants — only the surrounding `GRAPH ?g {...}` wrapper
/// changes, which the caller adds.
fn group_pattern() -> &'static str {
    "?group a dec:DispatchGroup ; \
            dec:dispatchStatus ?status ; \
            dec:hasActionSession ?action . \
     OPTIONAL { ?action dec:status ?actionStatus } \
     OPTIONAL { ?action dec:role ?actionRole } \
     OPTIONAL { ?action prov:atTime ?actionStartedAt } \
     OPTIONAL { \
       ?group dec:hasInterpretationSession ?interp . \
       ?verdict a dec:VerificationVerdict ; \
                prov:wasGeneratedBy ?interp ; \
                dec:verdict ?verdictValue . \
     }"
}

fn role_clause(role_filter: Option<&str>) -> String {
    match role_filter {
        Some(role) => format!(
            "FILTER( ?actionRole = \"{role}\" )",
            role = sparql_escape_literal(role),
        ),
        None => String::new(),
    }
}

fn window_clause(window: Option<(&str, &str)>) -> String {
    match window {
        Some((since, until)) => format!(
            "FILTER( BOUND(?actionStartedAt) && \
             xsd:dateTime(STR(?actionStartedAt)) >= \"{since}\"^^xsd:dateTime && \
             xsd:dateTime(STR(?actionStartedAt)) <= \"{until}\"^^xsd:dateTime )",
            since = sparql_escape_literal(since),
            until = sparql_escape_literal(until),
        ),
        None => String::new(),
    }
}

/// Escape a string literal for safe interpolation into a SPARQL
/// double-quoted literal body. The agreement public API rejects role
/// strings that fail role-catalog lookup *before* this is reached, so
/// the only inputs that hit this function are role ids and ISO-8601
/// timestamps — both already constrained — but the escape is here in
/// depth.
fn sparql_escape_literal(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unfiltered_query_compiles_to_a_select() {
        let q = build_query(None, None);
        assert!(q.contains("SELECT DISTINCT"));
        assert!(q.contains("dec:DispatchGroup"));
        assert!(q.contains("dec:hasInterpretationSession"));
        assert!(q.contains("dec:VerificationVerdict"));
        // No FILTER body when both filters are absent.
        assert!(!q.contains("FILTER"));
    }

    #[test]
    fn role_filter_emits_a_filter_clause() {
        let q = build_query(None, Some("implementer"));
        assert!(q.contains("FILTER( ?actionRole = \"implementer\" )"));
    }

    #[test]
    fn window_filter_emits_a_datetime_range_clause() {
        let q = build_query(Some(("2026-01-01T00:00:00Z", "2026-02-01T00:00:00Z")), None);
        assert!(q.contains("FILTER( BOUND(?actionStartedAt)"));
        assert!(q.contains("2026-01-01T00:00:00Z"));
        assert!(q.contains("2026-02-01T00:00:00Z"));
    }

    #[test]
    fn role_literal_is_escaped() {
        // Role strings shouldn't contain a quote in practice (role
        // catalog ids are kebab-case), but defence in depth: any quote
        // is escaped so the query body cannot be broken out of.
        let q = build_query(None, Some("foo\"bar"));
        assert!(q.contains("FILTER( ?actionRole = \"foo\\\"bar\" )"));
    }

    #[test]
    fn terminal_status_list_is_exhaustive() {
        // If the dispatch-status set grows, this assertion catches
        // metric drift — the metric module then needs an update too.
        let statuses = terminal_statuses();
        assert_eq!(statuses.len(), 5);
        assert!(statuses.contains(&DISPATCH_STATUS_COMPLETE));
        assert!(statuses.contains(&DISPATCH_STATUS_INTERPRETATION_REJECTED));
        assert!(statuses.contains(&DISPATCH_STATUS_AWAITING_AMENDMENT));
        assert!(statuses.contains(&DISPATCH_STATUS_ACTION_FAILED));
        assert!(statuses.contains(&DISPATCH_STATUS_INTERPRETATION_FAILED));
    }
}
