//! VerificationEnvironment artifact type per FT-035 / ADR-028.
//!
//! Per the slice-level SDP convention in `CLAUDE.md`, every consumer
//! (FT-037 safety enforcement, FT-038/039/040 env CLI subcommands)
//! imports from this module — no consumer imports from sibling features.
//!
//! Scope of FT-035 is intentionally the schema substrate: the in-memory
//! shape (`SafetyClass`, `VerificationEnvironment`), RDF serialisation
//! (`to_quads`), on-disk Turtle round-trip (`from_turtle`,
//! `to_canonical_turtle`), and the ADR-028 SHACL shape
//! (`validate_quads`). Safety enforcement (FT-037), CLI subcommands
//! (FT-038/039/040), and graph artifacts (FT-036) are out of scope here.

pub mod io;
pub mod seed;
pub mod shacl;
pub mod types;

#[cfg(test)]
mod tests;

pub use io::{from_turtle, from_turtle_bytes, to_canonical_turtle, EnvIoError};
pub use seed::{ephemeral_cli_env, EPHEMERAL_CLI_ENV_FILENAME, EPHEMERAL_CLI_ENV_ID};
pub use shacl::{validate_quads, EnvShaclError, EnvViolation, REMOTE_ENV_TYPE_PREFIX};
pub use types::{SafetyClass, VerificationEnvironment};
