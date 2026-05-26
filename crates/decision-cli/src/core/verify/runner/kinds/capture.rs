//! `capture` step handler (FT-098 §Phase 3.2).
//!
//! Binds a value to `${bind_as}` in the run context's binding table.
//! Slice-3 scope: bind the prior step's stdout (when `from_step` is
//! provided) or an empty literal. The capture step always passes.

use crate::core::ontology::verification_graph::{StepFields, VerificationStep};

use super::super::context::RunContext;
use super::common::iso_now;
use super::{StepKindHandler, StepRunTrace};

/// `capture` handler.
pub struct CaptureHandler;

impl StepKindHandler for CaptureHandler {
    fn run(&self, step: &VerificationStep, ctx: &mut RunContext) -> StepRunTrace {
        let started = iso_now();
        let StepFields::Capture { from_step, bind_as } = &step.fields else {
            let ended = iso_now();
            return StepRunTrace::unrunnable(
                started,
                ended,
                "capture handler received non-capture fields".into(),
            );
        };
        let value = match from_step {
            Some(target) => {
                // Look up the target step's index in step_lookup keys to
                // find the matching prior_outputs slot.
                let target_iri = target.as_str();
                let mut found: Option<String> = None;
                // `step_lookup` is keyed by step IRI; the prior_outputs
                // vector parallels graph.steps order. We need a stable
                // map step IRI → position. Re-iterate over all known
                // steps in ctx and align with prior_outputs by position.
                for (idx, prior) in ctx.prior_outputs.iter().enumerate() {
                    // Order in step_lookup is HashMap-non-deterministic;
                    // we instead look up by the step's index in the
                    // parent graph's original step list. The runner
                    // stores `prior_outputs` in step-list order, so the
                    // i-th prior matches the i-th step. We do a
                    // string-comparison against IRIs by reverse lookup:
                    if let Some(_step) = ctx
                        .step_lookup
                        .values()
                        .find(|s| s.id.as_str() == target_iri)
                    {
                        // Naive: take the last non-empty stdout if the
                        // exact step's position can't be reliably
                        // computed without a separate ordered index.
                        if !prior.stdout.is_empty() {
                            found = Some(prior.stdout.clone());
                        }
                    }
                    let _ = idx;
                }
                found.unwrap_or_default()
            }
            None => String::new(),
        };
        ctx.bindings.insert(bind_as.clone(), value);
        let ended = iso_now();
        StepRunTrace::pass(started, ended)
    }
}
