//! # dec-ontology — the decision-cli domain layer (ADR-086)
//!
//! Typed artifact definitions, IRI vocabulary modules, embedded SHACL
//! shape files, and the parser/emitter pairs that convert between quad
//! iterators and structs.
//!
//! This crate is the center of the workspace's stable dependency graph.
//! Its load-bearing property is what it *cannot* do: the dependency tree
//! contains no store, no async runtime, no IO, no CLI. It speaks
//! [`oxrdf`] model types natively — IRIs and quads are the domain
//! vocabulary (ADR-002) — but it can only describe graph content, never
//! touch a store. Store-backed readers, SHACL enforcement at the
//! GraphWriter chokepoint (ADR-041), and SPARQL execution live above
//! this crate in `dec-graph` / `decision-cli`.

pub mod ontology;
pub mod vocab;
