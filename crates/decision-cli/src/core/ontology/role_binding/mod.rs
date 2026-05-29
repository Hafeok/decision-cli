//! `dec:RoleBinding` artifact type per FT-055 / ADR-033 / ADR-034.
//!
//! Per the slice-level SDP convention in `CLAUDE.md`, downstream
//! consumers (FT-058 catalog bootstrap, FT-061 dispatcher resolution,
//! FT-062 escalation loop) import from this module — no consumer imports
//! from sibling features.
//!
//! Scope of FT-055 is the schema substrate: in-memory shapes
//! (`RoleBinding`, `EscalationStep`, `TriggerSignal`), RDF serialisation
//! (`to_quads`), SHACL validation (`validate_quads`), and the read API
//! for the active binding for a given role.
//!
//! Catalog content (the seed YAML driving bootstrap), dispatcher
//! resolution, the escalation loop, and worker-side consumption land in
//! [FT-058..FT-062].

pub mod read;
mod read_helpers;
pub mod shacl;
pub mod types;

pub use read::{active_for_role, all_for_role, list_all_active, RoleBindingReadError};
pub use shacl::{validate_quads, RoleBindingShaclError, RoleBindingViolation};
pub use types::{EscalationStep, RoleBinding, TriggerSignal};
