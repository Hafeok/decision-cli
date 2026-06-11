//! Chain-integrity dispatch gate (FT-047 / ADR-031).
//!
//! The gate fires inside `core::harness::dispatch` *after* artifact
//! resolution and *before* any worker is invoked. For every feature-
//! targeted dispatch it consults [`crate::verify::coverage`] and
//! refuses to dispatch when the target feature's TCs are not fully
//! covered by at least one `dec:VerificationGraph` — unless the caller
//! supplies a structured `WaiverIntent` whose reason clears the
//! ≥16-non-whitespace-character bar.
//!
//! When a waiver is accepted, the gate mints a `dec:CoverageWaiver`,
//! persists it through the [`crate::StreamWriter`] chokepoint, writes
//! the on-disk Turtle at `.dec/verify/waivers/<id>.ttl`, and returns
//! [`GateOutcome::Pass`] carrying the waiver IRI so the dispatching
//! verb can record it in the session's PROV-O chain.
//!
//! Listing / showing / revoking waivers (slice 3+) is **out of scope**:
//! this slice produces the artifacts that future verbs will surface.

mod gate;
mod waiver_intent;
mod writer;

pub use gate::{run_chain_integrity_gate, ChainIntegrityError, GateInputs, GateOutcome};
pub use waiver_intent::{validate_waiver_reason, WaiverIntent, WaiverReasonError};
pub use writer::{persist_waiver, NextWaiverIdResolver, WaiverPersistError};
