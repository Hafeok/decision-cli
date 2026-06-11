//! `dec:ConformanceAudit` artifact type per FT-092 / ADR-055 / ADR-060.
//!
//! A ConformanceAudit is the per-WorkerImage admission evidence the
//! WorkerCurator (FT-092) writes when admitting a Submission. The
//! audit's *class* discriminator distinguishes the evidence kind:
//!
//! - `manual-review` — slice 1 baseline per ADR-060. The Curator
//!   inspected the Submission's signature verdict, SBOM reference, and
//!   provenance claims and authored a notes field justifying admission.
//! - `automated-replay` — slice 2+. A conformance runner replayed the
//!   candidate image against a corpus of historical bundles with
//!   known-good artifacts; notes carries the run summary.
//!
//! The schema is the same across classes — only the runtime that
//! produces the audit changes. The ConformanceAudit artifact lives in
//! the orchestration graph alongside the WorkerImage it audits, and is
//! referenced from the WorkerImage's `dec:conformance_audit` predicate
//! (FT-086).
//!
//! Dual-provenance discipline (ADR-038) applies:
//!
//! - **mechanical**: `prov:wasGeneratedBy` → action session,
//!   `prov:wasAttributedTo` → agent, `prov:generatedAtTime` → RFC3339.
//! - **motivational**: `dec:audits` → audited WorkerImage (a
//!   `wasDerivedFrom` sub-property per ADR-039 / FT-070).

pub mod types;

pub use types::{ConformanceAudit, ConformanceAuditClass};
