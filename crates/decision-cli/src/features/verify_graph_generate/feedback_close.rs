//! FT-107 lifecycle-transition shim. The original implementation moved
//! to [`crate::core::feedback::address_walk`] in FT-108 so the
//! code-writer accept path could share the same helper without
//! crossing the slice-SDP boundary (features depend on `core/`, not on
//! each other). This file is now a thin re-export so existing callers
//! in `verify_graph_generate::*` continue to compile unchanged.

pub use crate::core::feedback::address_walk::{
    mark_addressed as mark_addressed_inner, mark_batch_addressed as mark_batch_addressed_inner,
    AddressWalkError,
};

/// Backward-compatible alias for the FT-107 error type.
pub type FeedbackCloseError = AddressWalkError;

/// Backward-compatible wrapper: prior signature took a `target_role_id`
/// parameter we no longer use (the runner's emitter already stamps the
/// canonical `dec:targetRole` when the feedback is born). Keep the
/// parameter on the surface so the verify_graph_generate dispatch path
/// doesn't need rewiring.
pub fn mark_addressed(
    store: &oxigraph::store::Store,
    writer: &crate::core::stream_writer::StreamWriter,
    feedback_iri: &str,
    addressing_artifact_iri: &oxigraph::model::NamedNode,
    _target_role_id: &str,
    session_iri: &oxigraph::model::NamedNode,
    now_rfc3339: &str,
) -> Result<(), AddressWalkError> {
    mark_addressed_inner(
        store,
        writer,
        feedback_iri,
        addressing_artifact_iri,
        session_iri,
        now_rfc3339,
    )
}

/// Backward-compatible batch wrapper. Same shape as the prior helper.
pub fn mark_batch_addressed(
    workdir: &std::path::Path,
    feedback_iris: &[String],
    addressing_artifact_iri: &oxigraph::model::NamedNode,
    _target_role_id: &str,
    session_iri: &oxigraph::model::NamedNode,
    now_rfc3339: &str,
) -> Result<usize, AddressWalkError> {
    mark_batch_addressed_inner(
        workdir,
        feedback_iris,
        addressing_artifact_iri,
        session_iri,
        now_rfc3339,
    )
}
