//! ADR-085 status gating (E020) and promotion-readiness walk (W104).

use oxigraph::model::{Quad, Term};
use oxigraph::sparql::QueryResults;
use oxigraph::store::Store;
use thiserror::Error;

use dec_ontology::ontology::archetype::ArchetypeStatus;
use dec_ontology::vocab::{
    IRI_DEC_APPLICATION_CONTRACT_HELD_INVARIANT, IRI_DEC_ARCHETYPE, IRI_DEC_ARCHETYPE_STATUS,
    IRI_DEC_COVERAGE_NOTE, IRI_DEC_SEAM_AUDIT,
};

/// Error code for status mutations outside the promote CLI path
/// (ADR-085 §6, mirroring the ADR-status E020 gate).
pub const E020_CODE: &str = "E020_ArchetypeStatusOutsidePromotePath";

/// Warning code for promotion-ready candidates (informational).
pub const W104_CODE: &str = "W104_ArchetypePromotionReady";

/// Who is asking to write an archetype status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusWriteAuthority {
    /// A regular write path (workers, bootstrap, migrations). May only
    /// register `candidate` / `quarantined`, or repeat the stored status.
    Standard,
    /// The `dec archetype promote` / `demote` CLI path (FT-158). The
    /// only authority allowed to change a stored status to `standard`.
    PromotePath,
}

/// Refusal raised by the ADR-085 §6 status gate.
#[derive(Debug, Error)]
#[error("{E020_CODE}: archetype <{subject}> status {from:?} → {to:?} refused: {detail}")]
pub struct StatusGateError {
    /// Archetype subject IRI.
    pub subject: String,
    /// Status currently in the store, if the archetype exists.
    pub from: Option<String>,
    /// Status the mutation tries to write.
    pub to: String,
    /// Human-readable refusal reason.
    pub detail: String,
}

/// Enforce ADR-085 §6 over an insert set: any quad that sets
/// `dec:status` to `standard` — or changes an existing stored status —
/// must come through [`StatusWriteAuthority::PromotePath`].
pub fn validate_status_transition_with_store(
    store: &Store,
    inserts: &[Quad],
    authority: StatusWriteAuthority,
) -> Result<(), StatusGateError> {
    if authority == StatusWriteAuthority::PromotePath {
        return Ok(());
    }
    for q in inserts {
        if q.predicate.as_str() != IRI_DEC_ARCHETYPE_STATUS {
            continue;
        }
        let to = match &q.object {
            Term::Literal(l) => l.value().to_string(),
            _ => continue,
        };
        let subject = q.subject.to_string();
        let stored = stored_status(store, &q.subject.to_string());

        let changes_stored = stored.as_deref().is_some_and(|s| s != to);
        let mints_standard = to == ArchetypeStatus::Standard.as_str()
            && stored.as_deref() != Some(ArchetypeStatus::Standard.as_str());

        if mints_standard || changes_stored {
            return Err(StatusGateError {
                subject,
                from: stored,
                to,
                detail: "status promotion/demotion is a gated human decision; route through \
                         the dec archetype promote/demote path (ADR-085 §6)"
                    .to_string(),
            });
        }
    }
    Ok(())
}

fn stored_status(store: &Store, subject_nt: &str) -> Option<String> {
    let q = format!(
        "SELECT ?s WHERE {{ GRAPH ?g {{ {subject_nt} <{IRI_DEC_ARCHETYPE_STATUS}> ?s }} }} LIMIT 1"
    );
    if let Ok(QueryResults::Solutions(mut sols)) = store.query(q.as_str()) {
        if let Some(Ok(sol)) = sols.next() {
            if let Some(Term::Literal(l)) = sol.get("s") {
                return Some(l.value().to_string());
            }
        }
    }
    None
}

/// One W104 finding: a candidate archetype whose recorded evidence
/// satisfies the FT-147-era readiness approximation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PromotionReadiness {
    /// Archetype subject IRI.
    pub archetype: String,
    /// Always [`W104_CODE`]; carried so renderers stay uniform.
    pub code: &'static str,
}

/// Walk `status: candidate` archetypes and report W104 for each whose
/// evidence holds (informational only, never blocking).
///
/// FT-147-era approximation of ADR-085 §1: the full four requirements
/// need the SeamAudit `monolith_bar` field (FT-152) and Instance
/// artifacts (FT-156), which do not exist yet. Until they land, "ready"
/// means: ≥1 seam audit linked, `applicationContractHeldInvariant`
/// true, and a non-empty `coverageNote`. FT-152/FT-156 tighten this
/// walk to the real evidence set.
#[must_use]
pub fn promotion_ready_candidates(store: &Store) -> Vec<PromotionReadiness> {
    let q = format!(
        "SELECT DISTINCT ?a WHERE {{ GRAPH ?g {{
            ?a a <{IRI_DEC_ARCHETYPE}> ;
               <{IRI_DEC_ARCHETYPE_STATUS}> \"candidate\" ;
               <{IRI_DEC_SEAM_AUDIT}> ?seam ;
               <{IRI_DEC_APPLICATION_CONTRACT_HELD_INVARIANT}> ?inv ;
               <{IRI_DEC_COVERAGE_NOTE}> ?note .
            FILTER(STR(?inv) = \"true\" && STRLEN(STR(?note)) > 0)
        }} }}"
    );
    let mut ready = Vec::new();
    if let Ok(QueryResults::Solutions(sols)) = store.query(q.as_str()) {
        for sol in sols.flatten() {
            if let Some(Term::NamedNode(a)) = sol.get("a") {
                ready.push(PromotionReadiness {
                    archetype: a.as_str().to_string(),
                    code: W104_CODE,
                });
            }
        }
    }
    ready
}
