//! Action executor — turns an `Action` into a side-effecting call
//! against the relevant feature handler.
//!
//! Kept narrow on purpose: the driver loop owns iteration counting,
//! history tracking, and Stuck/Done detection; this module's job is
//! the one-line dispatch per non-terminal `Action` variant.

use anyhow::{Context, Result};
use std::path::Path;

use crate::core::drive::{Action, PlanContext};

/// Trait the driver loop calls to side-effect on a non-terminal
/// action. Tests substitute a stub so the loop logic can be exercised
/// without spawning workers.
pub trait Executor {
    /// Dispatch the action. Returns `Ok(())` on success; errors
    /// propagate to the driver, which surfaces them as
    /// `DriveError::Execute`.
    fn execute(&mut self, ctx: &PlanContext, action: &Action) -> Result<()>;
}

/// Production executor — calls the real feature handlers.
pub struct ProductionExecutor;

impl Executor for ProductionExecutor {
    fn execute(&mut self, ctx: &PlanContext, action: &Action) -> Result<()> {
        match action {
            Action::Done | Action::Stuck { .. } => {
                // Terminal actions never reach the executor; the
                // driver short-circuits before calling us. Defensive.
                Ok(())
            }
            Action::DispatchVerifier { feature_id, env_id } => {
                run_verify_feature(&ctx.workdir, feature_id, Some(env_id))
                    .with_context(|| format!("dispatch verifier for {feature_id}"))
            }
            Action::DispatchImplementer { feature_id } => run_implement(&ctx.workdir, feature_id)
                .with_context(|| format!("dispatch implementer for {feature_id}")),
            Action::DispatchVerifyGraphAuthor { feature_id, env_id } => {
                run_verify_graph_generate(&ctx.workdir, feature_id, env_id)
                    .with_context(|| format!("dispatch verify-graph-author for {feature_id}"))
            }
            Action::EscalateVgaToImplementer { feature_id } => {
                escalate_and_dispatch_implementer(ctx, feature_id)
                    .with_context(|| format!("escalate vga→implementer for {feature_id}"))
            }
            Action::EscalateImplementerToVga { feature_id, env_id } => {
                escalate_and_dispatch_vga(ctx, feature_id, env_id)
                    .with_context(|| format!("escalate implementer→vga for {feature_id}"))
            }
            Action::DispatchTcAuthor { feature_id, .. } => {
                // FT-131: tc-author worker dispatch (stub for now)
                anyhow::bail!("DispatchTcAuthor not yet implemented for {feature_id}")
            }
            Action::DispatchCluster {
                feature_id,
                task_type_name,
            } => crate::features::drive::cluster_dispatch::run(ctx, feature_id, task_type_name)
                .with_context(|| format!("dispatch cluster {task_type_name} for {feature_id}")),
            Action::DispatchTcQuality { feature_id, .. } => {
                // FT-131: tc-quality judge dispatch (stub for now)
                anyhow::bail!("DispatchTcQuality not yet implemented for {feature_id}")
            }
            Action::DispatchVgQuality { feature_id, .. } => {
                // FT-131: vg-quality judge dispatch (stub for now)
                anyhow::bail!("DispatchVgQuality not yet implemented for {feature_id}")
            }
            Action::DispatchSpecAuthor { feature_id } => {
                // FT-131: spec-author worker dispatch (Slice B stub)
                anyhow::bail!("DispatchSpecAuthor not yet implemented for {feature_id}")
            }
            Action::DispatchAdrAuthor { feature_id, .. } => {
                // FT-131: adr-author worker dispatch (Slice B stub)
                anyhow::bail!("DispatchAdrAuthor not yet implemented for {feature_id}")
            }
        }
    }
}

/// Implementation of [`Action::EscalateVgaToImplementer`]. Re-routes
/// the feature's open verifier-targeted defects to the implementer
/// (per ADR-024 supersession + twin pattern), then dispatches the
/// implementer so it sees the rerouted feedback in its bundle.
fn escalate_and_dispatch_implementer(ctx: &PlanContext, feature_id: &str) -> Result<()> {
    use crate::core::feedback::supersede_misrouted::escalate_verifier_defects_to_implementer;
    use crate::core::verify::coverage::feature_resolver::{resolve_feature_tcs_short, tc_iri_for};

    let tc_shorts = resolve_feature_tcs_short(&ctx.product_root, feature_id)
        .with_context(|| format!("resolve TCs for {feature_id}"))?;
    let tc_iris: Vec<String> = tc_shorts.iter().map(|s| tc_iri_for(s)).collect();
    let n = escalate_verifier_defects_to_implementer(&ctx.workdir, &tc_iris)
        .with_context(|| format!("escalating verifier defects for {feature_id}"))?;
    tracing::info!(
        target: "dec::drive::escalate",
        feature = %feature_id,
        rerouted = n,
        "rerouted verifier-targeted defects to implementer before dispatch"
    );
    run_implement(&ctx.workdir, feature_id)
        .with_context(|| format!("dispatch implementer for {feature_id} (post-escalation)"))
}

/// Implementation of [`Action::EscalateImplementerToVga`]. Mirror of
/// the above: re-routes the feature's open implementer-targeted
/// defects to the verifier (per ADR-024 supersession + twin pattern),
/// then dispatches the verify-graph-author to re-author the test.
fn escalate_and_dispatch_vga(ctx: &PlanContext, feature_id: &str, env_id: &str) -> Result<()> {
    use crate::core::feedback::supersede_misrouted::escalate_implementer_defects_to_verifier;
    use crate::core::verify::coverage::feature_resolver::{resolve_feature_tcs_short, tc_iri_for};

    let tc_shorts = resolve_feature_tcs_short(&ctx.product_root, feature_id)
        .with_context(|| format!("resolve TCs for {feature_id}"))?;
    let tc_iris: Vec<String> = tc_shorts.iter().map(|s| tc_iri_for(s)).collect();
    let n = escalate_implementer_defects_to_verifier(&ctx.workdir, &tc_iris)
        .with_context(|| format!("escalating implementer defects for {feature_id}"))?;
    tracing::info!(
        target: "dec::drive::escalate",
        feature = %feature_id,
        rerouted = n,
        "rerouted implementer-targeted defects to verifier before dispatch"
    );
    run_verify_graph_generate(&ctx.workdir, feature_id, env_id)
        .with_context(|| format!("dispatch verify-graph-author for {feature_id} (post-escalation)"))
}

fn run_verify_feature(workdir: &Path, feature_id: &str, env: Option<&str>) -> Result<()> {
    use crate::features::verify_feature::{run, FeatureVerifyRequest};
    let req = FeatureVerifyRequest {
        feature_id: feature_id.to_string(),
        environment_id: env.map(str::to_string),
        no_feedback: false,
        include_stale: false,
        dry_run: false,
        workdir: Some(workdir.to_path_buf()),
    };
    let _ = run(&req).context("verify-feature handler")?;
    Ok(())
}

fn run_implement(workdir: &Path, feature_id: &str) -> Result<()> {
    use crate::features::implement::{run, ImplementArgs};
    let args = ImplementArgs::new(feature_id);
    let _ = run(workdir, &args).context("implement handler")?;
    Ok(())
}

fn run_verify_graph_generate(workdir: &Path, feature_id: &str, env: &str) -> Result<()> {
    use crate::features::verify_graph_generate::{run_generate, GenerateMode, GenerateRequest};
    let req = GenerateRequest {
        feature_id: feature_id.to_string(),
        environment_id: env.to_string(),
        mode: GenerateMode::Accept,
        workdir: Some(workdir.to_path_buf()),
        product_root: None,
    };
    let _ = run_generate(&req).context("verify-graph-generate handler")?;
    Ok(())
}
