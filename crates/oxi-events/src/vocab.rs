//! Framework vocabulary IRIs for `oxi-events`.
//!
//! Per ADR-001 the public vocabulary is strictly *mutations,
//! subscriptions, events, delivery*. DDD-specific concepts
//! (roles, bundles, sessions) MUST NOT appear here.
//!
//! IRI constants are intentionally undocumented individually — names
//! and namespaces speak for themselves.

#![allow(missing_docs)]

use oxigraph::model::NamedNodeRef;

pub const NS_OXI: &str = "https://decision-cli.dev/oxi-events/ns#";
pub const NS_PROV: &str = "http://www.w3.org/ns/prov#";
pub const NS_RDF: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#";
pub const NS_XSD: &str = "http://www.w3.org/2001/XMLSchema#";

pub const IRI_RDF_TYPE: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#type";
pub const IRI_XSD_INTEGER: &str = "http://www.w3.org/2001/XMLSchema#integer";
pub const IRI_XSD_DATETIME: &str = "http://www.w3.org/2001/XMLSchema#dateTime";

pub const IRI_OXI_EVENT: &str = "https://decision-cli.dev/oxi-events/ns#Event";
pub const IRI_OXI_MUTATION: &str = "https://decision-cli.dev/oxi-events/ns#Mutation";
pub const IRI_OXI_SUBSCRIPTION: &str = "https://decision-cli.dev/oxi-events/ns#Subscription";

pub const IRI_OXI_SEQ: &str = "https://decision-cli.dev/oxi-events/ns#seq";
pub const IRI_OXI_STATUS: &str = "https://decision-cli.dev/oxi-events/ns#status";
pub const IRI_OXI_EMITTED_AT: &str = "https://decision-cli.dev/oxi-events/ns#emittedAt";
pub const IRI_OXI_MATCHED_SUBSCRIPTION: &str =
    "https://decision-cli.dev/oxi-events/ns#matchedSubscription";
pub const IRI_OXI_PUBLISHED: &str = "https://decision-cli.dev/oxi-events/ns#published";
pub const IRI_OXI_ACTOR: &str = "https://decision-cli.dev/oxi-events/ns#actor";
pub const IRI_OXI_CAUSE: &str = "https://decision-cli.dev/oxi-events/ns#cause";
pub const IRI_OXI_COMMITTED_AT: &str = "https://decision-cli.dev/oxi-events/ns#committedAt";
pub const IRI_OXI_SEQ_COUNTER: &str = "https://decision-cli.dev/oxi-events/ns#seq-counter";
pub const IRI_OXI_CURRENT: &str = "https://decision-cli.dev/oxi-events/ns#current";

pub const IRI_OXI_SUB_ASK_QUERY: &str = "https://decision-cli.dev/oxi-events/ns#askQuery";
pub const IRI_OXI_SUB_SELECT_QUERY: &str = "https://decision-cli.dev/oxi-events/ns#selectQuery";
pub const IRI_OXI_SUB_TRIGGER: &str = "https://decision-cli.dev/oxi-events/ns#trigger";
pub const IRI_OXI_SUB_MODE: &str = "https://decision-cli.dev/oxi-events/ns#mode";
pub const IRI_OXI_SUB_HANDLER: &str = "https://decision-cli.dev/oxi-events/ns#handler";

pub const SUB_MODE_INLINE: &str = "inline";
pub const SUB_MODE_ASYNC: &str = "async";

pub const IRI_OXI_GRAPH_EVENTS: &str = "https://decision-cli.dev/oxi-events/ns/events";
pub const IRI_OXI_GRAPH_META: &str = "https://decision-cli.dev/oxi-events/ns/meta";
pub const IRI_OXI_GRAPH_SUBSCRIPTIONS: &str =
    "https://decision-cli.dev/oxi-events/ns/subscriptions";

pub const IRI_PROV_WAS_GENERATED_BY: &str = "http://www.w3.org/ns/prov#wasGeneratedBy";
pub const IRI_PROV_WAS_ATTRIBUTED_TO: &str = "http://www.w3.org/ns/prov#wasAttributedTo";

pub const STATUS_OK: &str = "ok";
pub const STATUS_FAILED: &str = "failed";

#[must_use]
pub fn rdf_type() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_RDF_TYPE)
}

#[must_use]
pub fn xsd_integer() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_XSD_INTEGER)
}

#[must_use]
pub fn xsd_datetime() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_XSD_DATETIME)
}

#[must_use]
pub fn events_graph() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_OXI_GRAPH_EVENTS)
}

#[must_use]
pub fn meta_graph() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_OXI_GRAPH_META)
}

#[must_use]
pub fn subscriptions_graph() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_OXI_GRAPH_SUBSCRIPTIONS)
}
