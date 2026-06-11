//! VerificationBench artifact type per FT-035 / ADR-028 — pure schema
//! substrate: the in-memory shape, RDF serialisation, canonical-Turtle
//! emission, and the ADR-028 SHACL shape (`validate_quads`).
//!
//! Turtle *parsing* (`from_turtle`, `from_turtle_bytes`) needs an RDF
//! parser and stays in decision-cli's `core::ontology::verification_bench`
//! glue module (ADR-086).

pub mod seed;
pub mod shacl;
mod shacl_list;
pub mod types;
pub mod write;

pub use seed::{ephemeral_cli_env, EPHEMERAL_CLI_ENV_FILENAME, EPHEMERAL_CLI_ENV_ID};
pub use shacl::{validate_quads, EnvShaclError, EnvViolation, REMOTE_BENCH_TYPE_PREFIX};
pub use types::{SafetyClass, VerificationBench};
pub use write::to_canonical_turtle;
