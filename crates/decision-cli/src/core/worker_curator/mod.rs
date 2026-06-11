//! WorkerCurator role — admit or reject a `dec:WorkerImageSubmission` (FT-092).
//!
//! Slice-1 ships a Level-0 (human-filled) Curator per ADR-060: no
//! automated conformance corpus exists yet, so admission rests on a
//! Curator's judgment over a structured bundle. This module owns three
//! concerns:
//!
//! 1. **Bundle assembly** ([`bundle`]) — gathers the focal Submission
//!    (FT-087), its paired SignatureVerdict (FT-090), the SBOM-shaped
//!    slice of the Curator bundle (FT-091), and the set of existing
//!    `dec:WorkerImage`s sharing capability tags with the candidate.
//! 2. **Verdict input** ([`verdict`]) — `CuratorVerdict` carries the
//!    Curator's decision (Admit / Reject) plus rationale.
//! 3. **Session materialisation** ([`session`]) — `run_curator_session`
//!    turns (bundle, verdict, session-context) into a `CuratorOutcome`
//!    that materialises the produced artifacts: on Admit, a
//!    `dec:WorkerImage` with `eligibility_status=qualified` plus a
//!    `dec:ConformanceAudit` of class `manual-review`; on Reject, a
//!    `dec:Feedback` artifact rooted at the Submission. Both outcomes
//!    update the Submission's lifecycle (`received → admitted` or
//!    `received → rejected`) and stamp the matching edge
//!    (`produced_workerimage` or `produced_feedback`).
//!
//! Per the slice-level SDP convention in `CLAUDE.md`, this module lives
//! under `core/` so the `features/` slice that wires `dec worker
//! curator …` (slice 2+) and FT-092's integration test can both import
//! from one place — no consumer reaches into a sibling feature.
//!
//! Dual-provenance discipline (ADR-038) applies to every produced
//! artifact: mechanical PROV-O (session, agent, timestamp) is uniform,
//! and the motivational edge on each artifact is type-specific
//! (`dec:audits` on ConformanceAudit; `dec:sourceArtifact` /
//! `dec:sourceSession` on Feedback; the WorkerImage's motivational
//! chain is preserved through the Submission via the `dec:produced_workerimage`
//! inverse edge surfaced by the Submission's BoundaryArtifact origin).

pub mod bundle;
pub mod session;
pub mod verdict;

#[cfg(test)]
mod tests;

pub use bundle::{assemble_curator_bundle, CuratorBundle, CuratorBundleError};
pub use session::{
    run_curator_session, AdmissionOutcome, CuratorOutcome, CuratorSessionContext,
    CuratorSessionError, RejectionOutcome, WORKER_AUTHOR_TARGET_ROLE, WORKER_CURATOR_AGENT_IRI,
    WORKER_CURATOR_ROLE_ID,
};
pub use verdict::CuratorVerdict;
