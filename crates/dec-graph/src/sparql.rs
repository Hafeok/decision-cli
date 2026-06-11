//! SPARQL term-extraction helpers shared across features (ADR-016).
//!
//! These tiny utilities used to be duplicated across `main.rs`,
//! `cli/status.rs`, and several feature modules. Centralising them in
//! `core` lets every feature import the same canonical form.

use oxigraph::model::Term;

/// Render a `Term` as a plain IRI string when it is a `NamedNode`,
/// otherwise fall back to the `Display` form.
#[must_use]
pub fn term_iri_string(t: &Term) -> String {
    match t {
        Term::NamedNode(n) => n.as_str().to_string(),
        other => other.to_string(),
    }
}

/// Render a `Term` as its plain literal value, falling back to the
/// `Display` form for non-literal terms.
#[must_use]
pub fn term_literal_string(t: &Term) -> String {
    match t {
        Term::Literal(lit) => lit.value().to_string(),
        Term::NamedNode(n) => n.as_str().to_string(),
        other => other.to_string(),
    }
}
