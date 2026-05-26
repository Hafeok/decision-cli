//! Vertical-slice feature modules — one directory per `dec` subcommand (ADR-016).
//!
//! Sibling features MUST NOT import from each other; shared code lives
//! in `core/`. The ADR-016 audit script (`scripts/checks/vertical-slice-
//! imports.sh`) enforces this structurally at build time, on top of
//! Rust's module visibility rules.

pub mod events;
pub mod feedback;
pub mod finalize;
pub mod ft_074_migrate_provenance;
pub mod health;
pub mod implement;
pub mod init;
pub mod mcp;
pub mod preflight;
pub mod session_inspect;
pub mod submissions;
pub mod telemetry;
pub mod verify_env_list;
pub mod verify_feature;
pub mod verify_env_new;
pub mod verify_env_show;
pub mod verify_graph_generate;
pub mod verify_graph_list;
pub mod verify_graph_new;
pub mod verify_graph_run;
pub mod verify_graph_show;
pub mod verify_step_add;
pub mod workers_run;
