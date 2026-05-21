//! FT-050 / ADR-030 — vocabulary for the verify-graph-author auto-dispatch subscription.
//!
//! Split out of `core::vocab` to keep per-file size within the ADR-013
//! 400-line ceiling. Re-exported from the parent module so callers
//! continue to import from `decision_cli::core::vocab`.

#![allow(missing_docs)]

use oxigraph::model::NamedNodeRef;

// --- Event class and predicates ---------------------------------------------

/// Class IRI for `dec:VerifyGraphAuthorDispatchEvent` (FT-050 §Outputs).
pub const IRI_DEC_VERIFY_GRAPH_AUTHOR_DISPATCH_EVENT: &str =
    "https://decision-cli.dev/ns#VerifyGraphAuthorDispatchEvent";

/// Stable `dec:eventClass` literal for verify-graph-author dispatch events.
pub const EVENT_CLASS_VERIFY_GRAPH_AUTHOR_DISPATCH: &str = "verify-graph-author-dispatch";

/// Stable role id the dispatched event targets.
pub const VERIFY_GRAPH_AUTHOR_TARGET_ROLE: &str = "verify-graph-author";

/// `dec:feature` predicate — link to the feature artifact the dispatch
/// is generating a proposal for. Uses the same short-id convention as
/// other feature references (`FT-NNN` IRIs).
pub const IRI_DEC_FEATURE_REF: &str = "https://decision-cli.dev/ns#feature";

/// `dec:environment` is already declared in `verify_graph.rs`; reused here
/// for completeness. The dispatch event carries it as the target env.

/// `dec:bundleHash` predicate — content hash of the assembled bundle.
pub const IRI_DEC_BUNDLE_HASH: &str = "https://decision-cli.dev/ns#bundleHash";

/// `dec:triggeredByEventId` predicate — IRI of the originating feature
/// create/update event the subscription reacted to.
pub const IRI_DEC_TRIGGERED_BY_EVENT_ID: &str = "https://decision-cli.dev/ns#triggeredByEventId";

// --- Pending-review session predicates --------------------------------------

/// `dec:proposalDocument` predicate — JSON string carrying the worker's
/// `GraphProposal` payload, attached to a `pending_review` Session.
pub const IRI_DEC_PROPOSAL_DOCUMENT: &str = "https://decision-cli.dev/ns#proposalDocument";

/// Session status literal carried by an auto-dispatched session waiting
/// for human acceptance.
pub const SESSION_STATUS_PENDING_REVIEW: &str = "pending_review";

// --- Dedup ledger predicates ------------------------------------------------

/// Class IRI for `dec:AutoDispatchLedgerEntry` — one row per
/// `(feature, env, last_dispatch_at)` triple.
pub const IRI_DEC_AUTO_DISPATCH_LEDGER_ENTRY: &str =
    "https://decision-cli.dev/ns#AutoDispatchLedgerEntry";

/// `dec:ledgerFeature` predicate — short feature id literal.
pub const IRI_DEC_LEDGER_FEATURE: &str = "https://decision-cli.dev/ns#ledgerFeature";

/// `dec:ledgerEnvironment` predicate — short env id literal.
pub const IRI_DEC_LEDGER_ENVIRONMENT: &str = "https://decision-cli.dev/ns#ledgerEnvironment";

/// `dec:lastDispatchAt` predicate — RFC3339 timestamp of the most recent
/// dispatch for the `(feature, env)` pair.
pub const IRI_DEC_LAST_DISPATCH_AT: &str = "https://decision-cli.dev/ns#lastDispatchAt";

/// Named graph IRI holding the auto-dispatch dedup ledger. Separate from
/// the orchestration graph so subscriptions can be queried in isolation.
pub const IRI_DEC_GRAPH_AUTO_DISPATCH_LEDGER: &str =
    "https://decision-cli.dev/ns/graph/auto-dispatch-ledger";

#[must_use]
pub fn verify_graph_author_dispatch_event_class() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_VERIFY_GRAPH_AUTHOR_DISPATCH_EVENT)
}

#[must_use]
pub fn feature_ref() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_FEATURE_REF)
}

#[must_use]
pub fn bundle_hash_pred() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_BUNDLE_HASH)
}

#[must_use]
pub fn triggered_by_event_id() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_TRIGGERED_BY_EVENT_ID)
}

#[must_use]
pub fn proposal_document() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_PROPOSAL_DOCUMENT)
}

#[must_use]
pub fn auto_dispatch_ledger_entry_class() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_AUTO_DISPATCH_LEDGER_ENTRY)
}

#[must_use]
pub fn ledger_feature() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_LEDGER_FEATURE)
}

#[must_use]
pub fn ledger_environment() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_LEDGER_ENVIRONMENT)
}

#[must_use]
pub fn last_dispatch_at() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_LAST_DISPATCH_AT)
}

#[must_use]
pub fn auto_dispatch_ledger_graph() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_GRAPH_AUTO_DISPATCH_LEDGER)
}
