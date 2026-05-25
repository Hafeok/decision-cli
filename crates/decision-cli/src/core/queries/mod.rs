//! QueryTemplate catalog accessors (FT-075 / ADR-043).
//!
//! Hosts the `QueryTemplate` artifact accessors and the slice-1 canonical
//! full-chain traversal templates. Per ADR-016, this module lives under
//! `core/` so every feature reads the catalog through here — never through
//! a sibling feature module.
//!
//! The shipped TTL fixtures under [`bootstrap`](self) are embedded via
//! `include_str!` and seeded into the orchestration store by `dec init`
//! (FT-009 extension). After init the graph is authoritative; the
//! fixtures act as documentation and rebootstrap input.

pub mod full_chain;

pub use full_chain::{
    bootstrap_query_template_quads, execute_template, fetch_query_template, list_query_templates,
    QueryTemplate, QueryTemplateError, FULL_CHAIN_BACKWARD_ID, FULL_CHAIN_BACKWARD_IRI,
    FULL_CHAIN_BACKWARD_TTL, FULL_CHAIN_FORWARD_ID, FULL_CHAIN_FORWARD_IRI, FULL_CHAIN_FORWARD_TTL,
    QUERY_TEMPLATE_CLASS_IRI,
};
