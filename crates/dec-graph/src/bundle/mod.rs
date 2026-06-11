//! `dec:Bundle` artifact type with stakes per FT-056 / ADR-035.
//!
//! Per the slice-level SDP convention in `CLAUDE.md`, downstream consumers
//! (dispatcher escalation, reasoning_effort mapping, bundle serialisation)
//! import from this module — no consumer imports from sibling features.
//!
//! Scope of FT-056 is the schema substrate: the `Stakes` enum, the
//! in-memory `Bundle` shape, the `BundleBuilder` (with `with_stakes`
//! override), the `default_stakes_for` ladder, RDF serialisation,
//! SHACL validation, and the bootstrap migration backfill.
//!
//! Dispatcher consumption (escalation triggers reading `stakes`) and the
//! `reasoning_effort` mapping live in their own feature modules — this
//! module just exposes the field.

pub mod default_ladder;
pub mod migration;
pub mod shacl;
pub mod types;

pub use default_ladder::{default_stakes_for, AdrScope, FocalContext};
pub use migration::{migrate_bundle_stakes, MigrationError, MigrationOutcome};
pub use shacl::{validate_quads, BundleShaclError, BundleViolation};
pub use types::{Bundle, BundleBuilder, Stakes, StakesParseError};
