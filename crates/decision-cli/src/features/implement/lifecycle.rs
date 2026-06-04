//! Post-dispatch lifecycle helpers extracted from [`super::run`].
//!
//! Splits the implementer-run pipeline into discrete steps:
//! payload assembly, code-change persistence, completion commit,
//! finalisation, outcome assembly. Each helper is small enough to read
//! at a glance per ADR-013 Rule 2.

use std::path::PathBuf;

use anyhow::{anyhow, Context, Result};
use chrono::Utc;
use oxi_events::Mutation;
use oxigraph::model::NamedNode;

use super::bundle::{persist_store, product_codechange_path};
use super::codechange::write_codechange_to_product_graph;
use super::quads::build_completion_quads;
use super::worker::{
    AuthorityJson, CodeChangeJson, DispatchPayloadJson, EscalationHintJson, WorkerResponseJson,
};
use super::{DispatchContext, ImplementArgs, ImplementOutcome, IMPLEMENTER_ROLE, SLICE1_MODEL_ID};
use crate::core::role_catalog;

/// Build the JSON payload streamed to the code-writer worker's stdin.
pub(super) fn build_dispatch_payload(
    ctx: &DispatchContext,
    args: &ImplementArgs,
) -> DispatchPayloadJson {
    let workspace_path = ctx
        .workspace_dir
        .canonicalize()
        .unwrap_or_else(|_| ctx.workspace_dir.clone())
        .to_string_lossy()
        .into_owned();
    let authority = lookup_role_authority(&ctx.store, IMPLEMENTER_ROLE);
    let defect_feedback = load_defect_feedback_for_feature(ctx, &args.feature_id);
    let allowed_tools = lookup_allowed_tools(&ctx.store, IMPLEMENTER_ROLE);
    DispatchPayloadJson {
        dispatch_id: ctx.dispatch_iri.as_str().to_string(),
        session_id: ctx.session_iri.as_str().to_string(),
        feature_id: args.feature_id.clone(),
        bundle_markdown: ctx.bundle_markdown.clone(),
        bundle_hash: ctx.bundle_hash.clone(),
        workspace_path,
        model_id: SLICE1_MODEL_ID.to_string(),
        // FT-066: until the implementer dispatch path is migrated to the
        // capability resolver (covered by a follow-on feature), pin to
        // the Anthropic endpoint so `claude -p` keeps its native API path.
        endpoint: "anthropic".to_string(),
        timeout_seconds: 1800,
        authority,
        defect_feedback,
        allowed_tools,
    }
}

/// FT-108: resolve the feature's TC short ids → IRIs and load any
/// `produced`-state, `class=defect`, `targetRole=implementer` feedback
/// targeting them. Best-effort: a missing product-root or resolve
/// failure degrades to an empty list rather than aborting the dispatch.
fn load_defect_feedback_for_feature(
    ctx: &DispatchContext,
    feature_id: &str,
) -> Vec<crate::core::feedback::DefectFeedbackRecord> {
    use crate::core::verify::coverage::feature_resolver::{resolve_feature_tcs_short, tc_iri_for};
    let tc_shorts = match resolve_feature_tcs_short(&ctx.product_root, feature_id) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };
    if tc_shorts.is_empty() {
        return Vec::new();
    }
    let tc_iris: Vec<String> = tc_shorts.iter().map(|t| tc_iri_for(t)).collect();
    // The dump path is <workdir>/.dec/store/orchestration.nq; the
    // loader wants <workdir>, so peel off three parents.
    let workdir = ctx
        .dump_path
        .parent()
        .and_then(|p| p.parent())
        .and_then(|p| p.parent());
    let Some(workdir) = workdir else {
        return Vec::new();
    };
    super::defect_feedback::load_for_implementer(workdir, &tc_iris)
}

/// FT-030: look up `role_id` in the orchestration store and project its
/// `dec:Authority` declaration into the wire shape consumed by workers.
/// Returns `None` on legacy stores that haven't been re-seeded.
fn lookup_role_authority(store: &oxigraph::store::Store, role_id: &str) -> Option<AuthorityJson> {
    let role = role_catalog::lookup(store, role_id).ok().flatten()?;
    let authority = role.authority?;
    Some(AuthorityJson {
        iri: authority.iri,
        may_decide: authority.may_decide,
        must_escalate: authority.must_escalate,
        escalate_via: authority
            .escalate_via
            .into_iter()
            .map(|h| EscalationHintJson {
                category: h.category,
                class: h.class.as_iri_value().to_string(),
                target_role: h.target_role,
            })
            .collect(),
        rationale: authority.rationale,
    })
}

/// FT-122 / ADR-070: look up the role's `dec:roleTool` set from the
/// catalog. Returns empty Vec on lookup failure or for legacy stores that
/// pre-date FT-121. The worker fail-closes on empty per ADR-069.
fn lookup_allowed_tools(store: &oxigraph::store::Store, role_id: &str) -> Vec<String> {
    match role_catalog::lookup(store, role_id) {
        Ok(Some(role)) => role.allowed_tools,
        Ok(None) => {
            tracing::warn!(role_id, "role not found in catalog; allowed_tools empty");
            Vec::new()
        }
        Err(e) => {
            tracing::warn!(role_id, error = %e, "role catalog lookup failed; allowed_tools empty");
            Vec::new()
        }
    }
}

/// Persist the worker's CodeChange into the product-cli graph slice.
pub(super) fn persist_code_change(
    ctx: &DispatchContext,
    args: &ImplementArgs,
    code_change: &CodeChangeJson,
) -> Result<PathBuf> {
    let codechange_path = product_codechange_path(&ctx.product_root);
    write_codechange_to_product_graph(
        &codechange_path,
        code_change,
        &ctx.session_iri,
        &ctx.dispatch_iri,
        &args.feature_id,
    )
    .with_context(|| {
        format!(
            "writing CodeChange artifact to product graph at {}",
            codechange_path.display()
        )
    })?;
    Ok(codechange_path)
}

/// Commit the session-completion quads (PROV-O `endedAtTime` + generated).
pub(super) fn commit_session_completion(
    ctx: &DispatchContext,
    code_change: &CodeChangeJson,
) -> Result<()> {
    let completed_at = Utc::now().to_rfc3339();
    let code_change_iri = NamedNode::new(&code_change.iri)
        .with_context(|| format!("code change IRI {}", code_change.iri))?;
    let complete_quads = build_completion_quads(&ctx.session_iri, &code_change_iri, &completed_at);
    let mut complete = Mutation::default();
    for q in complete_quads {
        complete.inserts.push(q);
    }
    ctx.writer
        .commit(complete.with_cause("dec implement: worker complete"))
        .context("committing session completion")?;
    persist_store(&ctx.store, &ctx.dump_path)?;
    Ok(())
}

/// Drive FT-017 finalisation (commit + status transition).
/// `defect_scoped` toggles the FT-110.X scope guard — when the
/// bundle carried defect feedback, modifications outside the
/// feature's prior commit history are rejected.
pub(super) fn finalize_implement_run(
    workdir: &std::path::Path,
    ctx: &DispatchContext,
    args: &ImplementArgs,
    code_change: &CodeChangeJson,
    defect_scoped: bool,
) -> Result<crate::finalize::FinalizeOutcome> {
    let scope_guard_extras = crate::finalize::load_scope_guard_extras(workdir);
    let finalize_input = crate::finalize::FinalizeInput {
        repo_root: workdir,
        product_root: &ctx.product_root,
        feature_id: &args.feature_id,
        session_iri: ctx.session_iri.as_str(),
        dispatch_iri: ctx.dispatch_iri.as_str(),
        code_change_iri: code_change.iri.as_str(),
        bundle_hash: &ctx.bundle_hash,
        worker_summary: &code_change.summary,
        defect_scoped,
        scope_guard_extras: &scope_guard_extras,
    };
    crate::finalize::finalize_run(&finalize_input).context("finalising dec implement run (FT-017)")
}

/// Assemble the final outcome returned by [`super::run`].
pub(super) fn assemble_implement_outcome(
    ctx: DispatchContext,
    response: &WorkerResponseJson,
    code_change: &CodeChangeJson,
    codechange_path: PathBuf,
    finalize_outcome: crate::finalize::FinalizeOutcome,
) -> ImplementOutcome {
    let files_written: Vec<PathBuf> = code_change
        .files
        .iter()
        .map(|f| ctx.workspace_dir.join(&f.path))
        .collect();
    let waiver_iri = ctx.waiver_iri.as_ref().map(|n| n.as_str().to_string());
    ImplementOutcome {
        session_iri: ctx.session_iri.as_str().to_string(),
        dispatch_iri: ctx.dispatch_iri.as_str().to_string(),
        code_change_iri: code_change.iri.clone(),
        bundle_hash: ctx.bundle_hash,
        workspace_dir: ctx.workspace_dir,
        product_codechange_path: codechange_path,
        files_written,
        worker_status: response.status.clone(),
        turn_count: response.telemetry.turn_count,
        latency_seconds: response.telemetry.latency_seconds,
        finalize: Some(finalize_outcome),
        waiver_iri,
    }
}

/// Resolve the worker response into a `&CodeChangeJson`, erroring when
/// the worker reported `status=ok` without producing a CodeChange.
pub(super) fn extract_code_change(response: &WorkerResponseJson) -> Result<&CodeChangeJson> {
    response
        .code_change
        .as_ref()
        .ok_or_else(|| anyhow!("worker reported status=ok with no code_change"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::role_catalog;
    use oxigraph::store::Store;

    /// TC-269: `build_dispatch_payload` populates `allowed_tools` from the
    /// role catalog lookup. Validates the FT-122 integration: given a store
    /// seeded by FT-121, the payload carries the implementer's tool surface.
    #[test]
    fn tc_269_build_dispatch_payload_carries_allowed_tools() {
        // Create a store and seed the implementer role
        let store = Store::new().expect("in-memory store");

        // Insert implementer role quads (includes allowed_tools from FT-121)
        for quad in role_catalog::seeds::implementer_seed_quads() {
            store.insert(&quad).expect("insert quad");
        }

        // Test the lookup helper directly
        let tools = lookup_allowed_tools(&store, IMPLEMENTER_ROLE);

        // Assert allowed_tools is populated from the catalog
        assert!(
            !tools.is_empty(),
            "allowed_tools should be populated from role catalog"
        );

        // The implementer role is seeded with these five tools (FT-121)
        let expected_tools = vec!["read_file", "write_file", "run_build", "run_lint", "run_tests"];
        for tool in &expected_tools {
            assert!(
                tools.contains(&tool.to_string()),
                "expected tool {tool} in allowed_tools, got {:?}",
                tools
            );
        }
        assert_eq!(
            tools.len(),
            expected_tools.len(),
            "allowed_tools should contain exactly the expected tools"
        );
    }
}
