//! Vertical-slice feature modules — one directory per `dec` subcommand (ADR-016).
//!
//! Sibling features MUST NOT import from each other; shared code lives
//! in `core/`. The ADR-016 audit script (`scripts/checks/vertical-slice-
//! imports.sh`) enforces this structurally at build time, on top of
//! Rust's module visibility rules.

pub mod events;
pub mod feedback;
pub mod finalize;
pub mod health;
pub mod implement;
pub mod init;
pub mod mcp;
pub mod session_inspect;
pub mod verify_env_new;
