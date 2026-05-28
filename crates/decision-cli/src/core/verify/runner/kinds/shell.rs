//! `shell-command` step handler (FT-098 §Phase 3.2).
//!
//! Spawns `bash -c <command>` in the run context's `dec_workdir`,
//! captures stdout/stderr (cap 4 KiB), and asserts the exit code.

use std::process::Command;

use crate::core::ontology::verification_graph::{StepFields, VerificationStep};

use super::super::context::{PriorOutput, RunContext};
use super::common::{cap_excerpt, iso_now};
use super::{StepKindHandler, StepRunTrace};

/// `shell-command` handler.
pub struct ShellHandler;

impl StepKindHandler for ShellHandler {
    fn run(&self, step: &VerificationStep, ctx: &mut RunContext) -> StepRunTrace {
        let started = iso_now();
        let StepFields::ShellCommand {
            command,
            expect_exit_code,
            ..
        } = &step.fields
        else {
            let ended = iso_now();
            return StepRunTrace::unrunnable(
                started,
                ended,
                "shell-command handler received non-shell fields".into(),
            );
        };
        let substituted = match ctx.substitute(command) {
            Ok(v) => v,
            Err(missing) => {
                let ended = iso_now();
                return StepRunTrace::unrunnable(
                    started,
                    ended,
                    format!("unbound capture: ${{{missing}}}"),
                );
            }
        };

        // Some shell-command steps carry a stop-on-fail signal via the
        // command text itself (slice-3 lacks a typed predicate). We
        // sniff a sentinel `#dec:stopOnFail` comment at the head; this
        // keeps the slice 2.5 step shape unchanged while allowing
        // TC-157 to exercise the behaviour.
        let stop_on_fail = substituted.starts_with("#dec:stopOnFail");

        let output = Command::new("bash")
            .arg("-c")
            .arg(&substituted)
            .current_dir(&ctx.dec_workdir)
            .env("TMPDIR", &ctx.dec_workdir)
            .env("DEC_WORKDIR", &ctx.dec_workdir)
            .env("DEC_VERIFY_TMP", &ctx.dec_workdir)
            // DEC_PROJECT_ROOT = the workdir the verifier was
            // launched from (the source tree). TCs whose runner-args
            // are relative paths (`tests/scripts/tc-XYZ.sh`) need
            // this to locate the script — the ephemeral
            // `dec_workdir` doesn't carry the repo's test scripts.
            .env("DEC_PROJECT_ROOT", &ctx.workdir)
            .output();
        let ended = iso_now();
        let result = match output {
            Ok(o) => o,
            Err(e) => {
                return StepRunTrace::unrunnable(
                    started,
                    ended,
                    format!("failed to spawn bash: {e}"),
                );
            }
        };
        let exit_code = result.status.code().unwrap_or(-1) as i64;
        let stdout = cap_excerpt(&result.stdout);
        let stderr = cap_excerpt(&result.stderr);
        let prior = PriorOutput {
            stdout: stdout.clone(),
            stderr: stderr.clone(),
            exit_code: Some(exit_code),
        };
        ctx.record_output(prior);
        let expected = expect_exit_code.unwrap_or(0);
        if exit_code == expected {
            StepRunTrace {
                outcome: crate::core::ontology::verification_result::StepOutcome::Pass,
                started_at: started,
                ended_at: ended,
                stdout_excerpt: stdout,
                stderr_excerpt: stderr,
                exit_code: Some(exit_code),
                error_message: String::new(),
                stop_on_fail,
            }
        } else {
            let msg = format!("expected exit {expected}, got {exit_code}");
            StepRunTrace {
                outcome: crate::core::ontology::verification_result::StepOutcome::Fail,
                started_at: started,
                ended_at: ended,
                stdout_excerpt: stdout,
                stderr_excerpt: stderr,
                exit_code: Some(exit_code),
                error_message: msg,
                stop_on_fail,
            }
        }
    }
}
