//! Catalog bootstrap from seed YAML (FT-058 / ADR-036).
//!
//! Materialises `dec:Capability` and `dec:RoleBinding` artifacts in the
//! orchestration store from YAML source files. The YAML is documentation
//! and bootstrap input; after the script runs, the graph is authoritative.
//! Strict divergence handling: when graph content disagrees with YAML
//! without a version bump, the script errors out and the transaction
//! rolls back. Bootstrap never silently overwrites graph state.

pub mod catalog;
pub mod diff;
pub mod migrate;
mod parse;
mod plan;
pub mod yaml;

pub use catalog::{bootstrap_catalog, BootstrapError, BootstrapReport};
pub use migrate::{migrate_bundle_stakes, migrate_session_token_breakdown};
