//! FT-092 / ADR-060 / ADR-055 — `dec:ConformanceAudit` vocabulary.
//!
//! A ConformanceAudit is the per-WorkerImage admission evidence the
//! WorkerCurator (FT-092) writes when admitting a Submission. Slice 1
//! ships only the `manual-review` audit class (ADR-060); slice 2+ adds
//! `automated-replay` against a conformance corpus. The schema is
//! identical across classes — the discriminator distinguishes the
//! evidence kind without schema churn.
//!
//! Per ADR-055, ConformanceAudit mirrors the Model-catalog evidence
//! shape: each audit records *what* it audited (`dec:audits` → the
//! audited `dec:WorkerImage`, motivational per ADR-039), *who* produced
//! it (mechanical PROV-O per ADR-038), and *what they observed*
//! (`dec:audit_notes`).

#![allow(missing_docs)]

use oxigraph::model::NamedNodeRef;

/// Class IRI for `dec:ConformanceAudit` (FT-092 / ADR-055 / ADR-060).
pub const IRI_DEC_CONFORMANCE_AUDIT_CLASS: &str =
    "https://decision-cli.dev/ns#ConformanceAudit";

/// IRI prefix for minted ConformanceAudit artifacts:
/// `https://decision-cli.dev/ns/conformance-audit/<id>`.
pub const IRI_DEC_CONFORMANCE_AUDIT_PREFIX: &str =
    "https://decision-cli.dev/ns/conformance-audit/";

/// `dec:audit_class` — one of {`manual-review`, `automated-replay`}.
pub const IRI_DEC_AUDIT_CLASS: &str = "https://decision-cli.dev/ns#audit_class";

/// `dec:audit_notes` — operator-facing free-form notes captured by the
/// audit producer (the WorkerCurator for `manual-review`; the conformance
/// runner for `automated-replay`).
pub const IRI_DEC_AUDIT_NOTES: &str = "https://decision-cli.dev/ns#audit_notes";

/// `dec:audits` motivational predicate — ConformanceAudit → WorkerImage
/// that the audit refers to. Declared as `rdfs:subPropertyOf
/// prov:wasDerivedFrom` in the motivational-predicates shape (ADR-039
/// / FT-070).
pub const IRI_DEC_AUDITS: &str = "https://decision-cli.dev/ns#audits";

/// Manual-review audit class literal (slice 1 baseline per ADR-060).
pub const CONFORMANCE_AUDIT_MANUAL_REVIEW: &str = "manual-review";

/// Automated-replay audit class literal (slice 2+; not produced by
/// slice 1 substrate).
pub const CONFORMANCE_AUDIT_AUTOMATED_REPLAY: &str = "automated-replay";

#[must_use]
pub fn conformance_audit_class() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_CONFORMANCE_AUDIT_CLASS)
}

#[must_use]
pub fn audit_class_pred() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_AUDIT_CLASS)
}

#[must_use]
pub fn audit_notes_pred() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_AUDIT_NOTES)
}

#[must_use]
pub fn audits_pred() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_AUDITS)
}
