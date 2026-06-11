//! `dec:CoverageWaiver` artifact type per FT-047 / ADR-031.
//!
//! The chain-integrity dispatch gate writes one of these every time a
//! caller invokes a feature-targeted dispatch with `--waive-coverage`
//! against a feature whose TCs are not fully covered by a verification
//! graph. The waiver is the on-disk audit record that makes the escape
//! hatch tolerable: each one carries the reason, the snapshot of which
//! TCs were uncovered when the gate fired, and PROV-O attribution.
//!
//! Scope is intentionally narrow:
//!
//!   * the in-memory shape (`CoverageWaiver`),
//!   * RDF serialisation (`to_quads`),
//!   * on-disk Turtle round-trip (`to_canonical_turtle`),
//!   * SHACL validation (`validate_quads`).
//!
//! Listing / showing / revoking waivers (slice 3+) and the waiver-rate
//! fitness function are explicitly out of scope.

pub mod shacl;
pub mod types;

pub use shacl::{validate_quads, WaiverShaclError, WaiverViolation};
pub use types::{CoverageWaiver, WaiverIoError};
