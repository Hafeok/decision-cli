//! IRI-keyed lookup for `dec:Capability` artifacts (FT-061 §Behaviour).
//!
//! The dispatcher's capability resolver walks a `dec:RoleBinding`'s
//! `dec:default_capability` pointer (an IRI) to the concrete
//! `dec:Capability` artifact. Unlike [`super::read::query_active_by_id`]
//! — which filters on `dec:status = "active"` so callers cannot
//! accidentally bind to an end-of-life capability — this lookup
//! returns the artifact whatever its status, so the resolver can
//! detect an EOL pointer and emit `ResolverError::CapabilityEol`.

use oxigraph::model::{NamedNode, Term};
use oxigraph::store::Store;

use dec_ontology::vocab::IRI_DEC_CAPABILITY;

use super::read::{collect_quads, parse_capability, CapabilityReadError};
use super::types::Capability;

/// Return the `dec:Capability` at `iri` regardless of its `dec:status`.
///
/// Returns `Ok(None)` when no artifact with `rdf:type dec:Capability`
/// exists at that IRI. Returns an error on graph-read failure or when
/// the artifact exists but does not match the FT-054 schema.
pub fn query_by_iri(
    store: &Store,
    iri: &NamedNode,
) -> Result<Option<Capability>, CapabilityReadError> {
    let quads = collect_quads(store, iri)?;
    if quads.is_empty() {
        return Ok(None);
    }
    let typed = quads.iter().any(|q| {
        q.predicate.as_str() == "http://www.w3.org/1999/02/22-rdf-syntax-ns#type"
            && matches!(&q.object, Term::NamedNode(n) if n.as_str() == IRI_DEC_CAPABILITY)
    });
    if !typed {
        return Ok(None);
    }
    Ok(Some(parse_capability(iri.as_str(), &quads)?))
}
