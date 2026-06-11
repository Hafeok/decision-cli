//! Store-facing glue for `dec:RoleBinding` (FT-055 / ADR-033 / ADR-034).
//!
//! The pure schema substrate lives in
//! [`dec_ontology::ontology::role_binding`] (ADR-086); this module keeps
//! the store-backed read APIs and re-exports both halves.

pub use dec_ontology::ontology::role_binding::*;

pub mod read;
mod read_helpers;

pub use read::{active_for_role, all_for_role, list_all_active, RoleBindingReadError};
