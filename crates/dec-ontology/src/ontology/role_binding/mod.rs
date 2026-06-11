//! `dec:RoleBinding` artifact type per FT-055 / ADR-033 / ADR-034 —
//! pure schema substrate: in-memory shapes, RDF serialisation
//! (`to_quads`), and SHACL validation (`validate_quads`).
//!
//! Store-backed read APIs (`active_for_role` et al.) live in
//! decision-cli's `core::ontology::role_binding` glue module (ADR-086).

pub mod shacl;
pub mod types;

pub use shacl::{validate_quads, RoleBindingShaclError, RoleBindingViolation};
pub use types::{EscalationStep, RoleBinding, TriggerSignal};
