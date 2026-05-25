//! Cutover routine — FT-074 §Behaviour step 6.
//!
//! Toggles the orchestration-store warn-only-mode flag from `true`
//! (the migration window's initial state) to `false` (reject mode)
//! once the unrepaired-orphan count drops at or below the operator-
//! supplied threshold. Returns `Err` when the count is above threshold;
//! the error body lists each unrepaired orphan IRI so the operator can
//! act on it directly.

#![allow(missing_docs)]

use anyhow::{anyhow, Result};
use oxigraph::model::NamedNode;
use oxigraph::sparql::QueryResults;
use oxigraph::store::Store;

use super::orphan_feedback::IRI_DEC_IS_MIGRATION_ORPHAN;

/// IRI of the orchestration-store flag that toggles GraphWriter between
/// warn-only and reject mode. Set by [`set_warn_only_mode`].
pub const IRI_DEC_WARN_ONLY_MODE: &str = "https://decision-cli.dev/ns#warnOnlyMode";

/// Stable subject IRI for the GraphWriter validator config record.
pub const IRI_DEC_VALIDATOR_CONFIG_SUBJECT: &str =
    "https://decision-cli.dev/ns/config/graphwriter-validator";

/// Result of a cutover attempt. Mirrors the structured behaviour the
/// CLI surface needs: success on transition, error with orphan count
/// when the threshold is breached.
#[derive(Debug, Clone)]
pub struct CutoverOutcome {
    pub orphan_count: usize,
    pub unrepaired_orphans: Vec<String>,
    pub warn_only_after: bool,
    pub flipped: bool,
}

/// Count unrepaired orphans by inspecting `:isMigrationOrphan true`
/// annotations. An operator who repairs an orphan removes the
/// annotation (slice-1 test simulates this).
pub fn count_unrepaired_orphans(store: &Store) -> Result<Vec<String>> {
    let sparql = format!(
        "SELECT DISTINCT ?a WHERE {{ \
           {{ ?a <{p}> ?v . FILTER(?v = true || str(?v) = \"true\") }} \
           UNION \
           {{ GRAPH ?g {{ ?a <{p}> ?v . FILTER(?v = true || str(?v) = \"true\") }} }} \
         }}",
        p = IRI_DEC_IS_MIGRATION_ORPHAN,
    );
    let mut out = Vec::new();
    if let QueryResults::Solutions(sols) = store.query(sparql.as_str())? {
        for sol in sols.flatten() {
            if let Some(oxigraph::model::Term::NamedNode(a)) = sol.get("a") {
                out.push(a.as_str().to_string());
            }
        }
    }
    Ok(out)
}

/// Set the orchestration-store warn-only-mode flag (idempotent). When
/// `enabled` is `true`, GraphWriter's validator runs in warn-only mode;
/// when `false`, the cutover flips to reject mode.
pub fn set_warn_only_mode(store: &Store, enabled: bool) -> Result<()> {
    let subject = NamedNode::new_unchecked(IRI_DEC_VALIDATOR_CONFIG_SUBJECT);
    let pred = NamedNode::new_unchecked(IRI_DEC_WARN_ONLY_MODE);
    let g = oxigraph::model::GraphName::NamedNode(NamedNode::new_unchecked(
        crate::core::vocab::IRI_DEC_GRAPH_ORCHESTRATION,
    ));
    let value = if enabled { "true" } else { "false" };
    let lit = oxigraph::model::Literal::new_typed_literal(
        value,
        NamedNode::new_unchecked("http://www.w3.org/2001/XMLSchema#boolean"),
    );
    let delete_q = format!(
        "DELETE {{ GRAPH <{g}> {{ <{s}> <{p}> ?o }} }} \
         WHERE  {{ GRAPH <{g}> {{ <{s}> <{p}> ?o }} }}",
        g = crate::core::vocab::IRI_DEC_GRAPH_ORCHESTRATION,
        s = IRI_DEC_VALIDATOR_CONFIG_SUBJECT,
        p = IRI_DEC_WARN_ONLY_MODE,
    );
    store
        .update(delete_q.as_str())
        .map_err(|e| anyhow!("clearing warn-only flag: {e}"))?;
    let quad = oxigraph::model::Quad::new(subject, pred, lit, g);
    store
        .transaction(|mut tx| tx.insert(quad.as_ref()))
        .map_err(|e| anyhow!("setting warn-only flag: {e}"))?;
    Ok(())
}

/// Query the current warn-only-mode value. Defaults to `true` when the
/// flag has never been written (the migration window's initial state).
pub fn warn_only_mode(store: &Store) -> Result<bool> {
    let sparql = format!(
        "SELECT ?v WHERE {{ \
           {{ <{s}> <{p}> ?v }} UNION {{ GRAPH ?g {{ <{s}> <{p}> ?v }} }} \
         }}",
        s = IRI_DEC_VALIDATOR_CONFIG_SUBJECT,
        p = IRI_DEC_WARN_ONLY_MODE,
    );
    let mut got = None;
    if let QueryResults::Solutions(sols) = store.query(sparql.as_str())? {
        for sol in sols.flatten() {
            if let Some(oxigraph::model::Term::Literal(lit)) = sol.get("v") {
                got = Some(lit.value() == "true");
                break;
            }
        }
    }
    Ok(got.unwrap_or(true))
}

/// Run the cutover sub-command. Returns `Err` when the orphan count is
/// above `threshold`; the error body lists each unrepaired orphan IRI.
pub fn run_cutover(store: &Store, threshold: usize) -> Result<CutoverOutcome> {
    let unrepaired = count_unrepaired_orphans(store)?;
    if unrepaired.len() > threshold {
        let mut msg = format!(
            "cutover refused: {} unrepaired orphan(s) > threshold {}:",
            unrepaired.len(),
            threshold
        );
        for iri in &unrepaired {
            msg.push_str("\n  • ");
            msg.push_str(iri);
        }
        return Err(anyhow!(msg));
    }
    set_warn_only_mode(store, false)?;
    let warn_only_after = warn_only_mode(store)?;
    Ok(CutoverOutcome {
        orphan_count: unrepaired.len(),
        unrepaired_orphans: unrepaired,
        warn_only_after,
        flipped: !warn_only_after,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup() -> Store {
        Store::new().expect("in-memory store")
    }

    #[test]
    fn warn_only_defaults_true_when_unset() {
        let store = setup();
        assert!(warn_only_mode(&store).expect("query"));
    }

    #[test]
    fn set_warn_only_mode_round_trips() {
        let store = setup();
        set_warn_only_mode(&store, false).expect("set");
        assert!(!warn_only_mode(&store).expect("query"));
        set_warn_only_mode(&store, true).expect("set");
        assert!(warn_only_mode(&store).expect("query"));
    }
}
