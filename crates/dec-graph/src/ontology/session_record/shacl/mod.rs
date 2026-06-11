//! Write-side SHACL validation for `dec:SessionRecord` (FT-057).
//!
//! Implements the three FT-057 `sh:sparql` consistency constraints:
//!
//! 1. **Bidirectional escalation**: `S1 dec:escalated_to S2` ⇔
//!    `S2 dec:escalated_from S1` — checked within the candidate quad set
//!    so the mutation is rejected unless *both* directions are present.
//! 2. **escalation_reason ↔ escalated_from coupling**: each is required
//!    iff the other is present.
//! 3. **Scaleway-no-cache**: if the session's resolving capability has
//!    `endpoint = scaleway`, the cache-write and cache-hit token counts
//!    must both be 0. Validation looks up the capability's endpoint in
//!    the candidate quad set first, then in the persistent store (when
//!    provided), so seeded catalogs feed the invariant.

mod checks;
mod endpoints;
mod helpers;

use std::collections::HashMap;

use oxigraph::model::{NamedNode, Quad, Subject, Term};
use oxigraph::store::Store;
use thiserror::Error;

use dec_ontology::vocab::IRI_DEC_SESSION_CAPABILITY;

use super::types::{IRI_DEC_SESSION, RDF_TYPE};

pub use helpers::SessionRecordViolation;

/// Structured failure for FT-057 SHACL validation.
#[derive(Debug, Error)]
#[error("SHACL validation failed for SessionRecord:\n{report}")]
pub struct SessionRecordShaclError {
    /// Rendered report (one `subject / path / detail` line per violation).
    pub report: String,
    /// The raw violations, in input order.
    pub violations: Vec<SessionRecordViolation>,
}

/// Run the FT-057 SHACL constraints against every `dec:Session` subject
/// declared in `quads`. Cross-subject lookups (bidirectional escalation,
/// Scaleway-no-cache via the capability's endpoint) are restricted to
/// the candidate quad set — see [`validate_quads_with_store`] for the
/// variant that also consults the persistent store for capabilities
/// declared in a prior mutation.
pub fn validate_quads(quads: &[Quad]) -> Result<(), SessionRecordShaclError> {
    validate_quads_with_store(quads, None)
}

/// Variant of [`validate_quads`] that consults `store` for capability
/// endpoint resolution when the capability is not declared inline in
/// `quads`. Used by `StreamWriter` so seeded catalogs feed the
/// Scaleway-no-cache invariant.
pub fn validate_quads_with_store(
    quads: &[Quad],
    store: Option<&Store>,
) -> Result<(), SessionRecordShaclError> {
    let subjects = session_subjects(quads);
    let mut violations: Vec<SessionRecordViolation> = Vec::new();
    let mut endpoints: HashMap<String, String> = endpoints::capability_endpoints(quads);
    if let Some(store) = store {
        endpoints::merge_store_endpoints(quads, &subjects, &mut endpoints, store);
    }
    for subject in &subjects {
        checks::check_escalation_reason_iff_from(quads, subject, &mut violations);
        checks::check_bidirectional_escalation(quads, subject, &mut violations);
        checks::check_escalation_reason_vocabulary(quads, subject, &mut violations);
        checks::check_token_fields_non_negative(quads, subject, &mut violations);
        checks::check_scaleway_no_cache(quads, subject, &endpoints, &mut violations);
    }
    if violations.is_empty() {
        return Ok(());
    }
    Err(SessionRecordShaclError {
        report: helpers::render_violations(&violations),
        violations,
    })
}

fn session_subjects(quads: &[Quad]) -> Vec<NamedNode> {
    let mut out: Vec<NamedNode> = Vec::new();
    for q in quads {
        if q.predicate.as_str() != RDF_TYPE {
            continue;
        }
        let Term::NamedNode(cls) = &q.object else {
            continue;
        };
        if cls.as_str() != IRI_DEC_SESSION {
            continue;
        }
        let Subject::NamedNode(s) = &q.subject else {
            continue;
        };
        if !out.iter().any(|n| n == s) {
            out.push(s.clone());
        }
    }
    out
}

// Silence unused-import warning when `IRI_DEC_SESSION_CAPABILITY` is only
// referenced indirectly via the helpers module.
#[allow(dead_code)]
const _PRED_GUARD: &str = IRI_DEC_SESSION_CAPABILITY;
