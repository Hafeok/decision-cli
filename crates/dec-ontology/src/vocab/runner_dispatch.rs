//! FT-100 — vocabulary for the verify-graph-runner auto-dispatch subscriptions.
//!
//! Split out of `core::vocab` to keep per-file size within the ADR-013
//! 400-line ceiling. Re-exported from the parent module so callers
//! continue to import from `decision_cli::core::vocab`.

#![allow(missing_docs)]

use oxrdf::NamedNodeRef;

// --- Event classes ---------------------------------------------------------

/// Class IRI for `dec:VerifyGraphRunDispatchEvent` (FT-100 §Outputs).
pub const IRI_DEC_VERIFY_GRAPH_RUN_DISPATCH_EVENT: &str =
    "https://decision-cli.dev/ns#VerifyGraphRunDispatchEvent";

/// Stable `dec:eventClass` literal for verify-graph-run dispatch events.
pub const EVENT_CLASS_VERIFY_GRAPH_RUN_DISPATCH: &str = "verify-graph-run-dispatch";

/// Stable role id the dispatched event targets.
pub const VERIFY_GRAPH_RUNNER_TARGET_ROLE: &str = "verify-graph-runner";

/// Stable session role for one per-`(graph, env)` runner session.
pub const SESSION_ROLE_VERIFY_GRAPH_RUNNER: &str = "verify-graph-runner";

/// Stable session role for the FT-100 aggregate (code-change-committed) session.
pub const SESSION_ROLE_VERIFY_GRAPH_RUNNER_AGGREGATE: &str = "verify-graph-runner-aggregate";

// --- Predicates ------------------------------------------------------------

/// `dec:verifyGraph` predicate — link to the `dec:VerificationGraph` being run.
pub const IRI_DEC_VERIFY_GRAPH_REF: &str = "https://decision-cli.dev/ns#verifyGraph";

/// `dec:triggerKind` predicate — what triggered the dispatch
/// (`graph-accepted` | `code-change-committed`).
pub const IRI_DEC_TRIGGER_KIND: &str = "https://decision-cli.dev/ns#triggerKind";

/// `dec:codeChange` predicate — link to the `dec:CodeChange` IRI when
/// the trigger is `code-change-committed`.
pub const IRI_DEC_CODE_CHANGE: &str = "https://decision-cli.dev/ns#codeChange";

/// `dec:aggregateVerdict` predicate — `approved` | `rejected` |
/// `amendment-required` set on the aggregate session.
pub const IRI_DEC_AGGREGATE_VERDICT: &str = "https://decision-cli.dev/ns#aggregateVerdict";

/// `dec:runActivity` predicate — IRI of the runner-side `prov:Activity`.
pub const IRI_DEC_RUN_ACTIVITY: &str = "https://decision-cli.dev/ns#runActivity";

/// `dec:partialFailureReasons` predicate — free-form notes on partial
/// failures (e.g. graph deleted between dispatch and run).
pub const IRI_DEC_PARTIAL_FAILURE_REASONS: &str =
    "https://decision-cli.dev/ns#partialFailureReasons";

// --- Trigger-kind literals -------------------------------------------------

/// `dec:triggerKind` literal for the `graph_accepted_dispatch` path.
pub const TRIGGER_KIND_GRAPH_ACCEPTED: &str = "graph-accepted";

/// `dec:triggerKind` literal for the `code_change_committed_dispatch` path.
pub const TRIGGER_KIND_CODE_CHANGE_COMMITTED: &str = "code-change-committed";

// --- Dedup ledgers ---------------------------------------------------------

/// Class IRI for `dec:GraphAcceptedLedgerEntry` — one row per
/// `(graph_iri, env_iri, last_dispatch_at)` triple.
pub const IRI_DEC_GRAPH_ACCEPTED_LEDGER_ENTRY: &str =
    "https://decision-cli.dev/ns#GraphAcceptedLedgerEntry";

/// Class IRI for `dec:CodeChangeCommittedLedgerEntry` — one row per
/// `(code_change_iri, feature_iri, last_dispatch_at)` triple.
pub const IRI_DEC_CODE_CHANGE_COMMITTED_LEDGER_ENTRY: &str =
    "https://decision-cli.dev/ns#CodeChangeCommittedLedgerEntry";

/// `dec:ledgerGraph` predicate — `(graph_iri)` short id on the
/// graph_accepted ledger row.
pub const IRI_DEC_LEDGER_GRAPH: &str = "https://decision-cli.dev/ns#ledgerGraph";

/// `dec:ledgerCodeChange` predicate — `code_change_iri` literal on the
/// code_change ledger row.
pub const IRI_DEC_LEDGER_CODE_CHANGE: &str = "https://decision-cli.dev/ns#ledgerCodeChange";

/// Named graph IRI holding the FT-100 graph-accepted dedup ledger.
pub const IRI_DEC_GRAPH_GRAPH_ACCEPTED_LEDGER: &str =
    "https://decision-cli.dev/ns/graph/graph-accepted-ledger";

/// Named graph IRI holding the FT-100 code-change-committed dedup ledger.
pub const IRI_DEC_GRAPH_CODE_CHANGE_LEDGER: &str =
    "https://decision-cli.dev/ns/graph/code-change-committed-ledger";

#[must_use]
pub fn verify_graph_run_dispatch_event_class() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_VERIFY_GRAPH_RUN_DISPATCH_EVENT)
}

#[must_use]
pub fn verify_graph_ref() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_VERIFY_GRAPH_REF)
}

#[must_use]
pub fn trigger_kind_pred() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_TRIGGER_KIND)
}

#[must_use]
pub fn code_change_pred() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_CODE_CHANGE)
}

#[must_use]
pub fn aggregate_verdict_pred() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_AGGREGATE_VERDICT)
}

#[must_use]
pub fn run_activity_pred() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_RUN_ACTIVITY)
}

#[must_use]
pub fn partial_failure_reasons_pred() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_PARTIAL_FAILURE_REASONS)
}

#[must_use]
pub fn graph_accepted_ledger_entry_class() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_GRAPH_ACCEPTED_LEDGER_ENTRY)
}

#[must_use]
pub fn code_change_committed_ledger_entry_class() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_CODE_CHANGE_COMMITTED_LEDGER_ENTRY)
}

#[must_use]
pub fn ledger_graph_pred() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_LEDGER_GRAPH)
}

#[must_use]
pub fn ledger_code_change_pred() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_LEDGER_CODE_CHANGE)
}

#[must_use]
pub fn graph_accepted_ledger_graph() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_GRAPH_GRAPH_ACCEPTED_LEDGER)
}

#[must_use]
pub fn code_change_ledger_graph() -> NamedNodeRef<'static> {
    NamedNodeRef::new_unchecked(IRI_DEC_GRAPH_CODE_CHANGE_LEDGER)
}
