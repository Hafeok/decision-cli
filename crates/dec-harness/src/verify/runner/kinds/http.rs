//! `http-request` step handler (FT-098 §Phase 3.2).
//!
//! Performs a synchronous (blocking) HTTP request through `reqwest`'s
//! blocking client. The runner is single-threaded per FT-098
//! §Idempotency, so blocking I/O is preferable to async overhead.

use std::time::Duration;

use dec_graph::ontology::verification_graph::{StepFields, VerificationStep};

use super::super::context::RunContext;
use super::common::iso_now;
use super::{StepKindHandler, StepRunTrace};

/// `http-request` handler.
pub struct HttpHandler;

impl StepKindHandler for HttpHandler {
    fn run(&self, step: &VerificationStep, ctx: &mut RunContext) -> StepRunTrace {
        let started = iso_now();
        let StepFields::HttpRequest {
            method,
            url,
            expect_status,
        } = &step.fields
        else {
            let ended = iso_now();
            return StepRunTrace::unrunnable(
                started,
                ended,
                "http-request handler received non-http fields".into(),
            );
        };
        let url = match ctx.substitute(url) {
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
        let method = method.trim().to_ascii_uppercase();
        // Use a thread to avoid touching tokio's runtime when running
        // inside another async context; the blocking client requires a
        // dedicated tokio runtime, so we spin one up ad-hoc.
        let rt = match tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(rt) => rt,
            Err(e) => {
                let ended = iso_now();
                return StepRunTrace::unrunnable(
                    started,
                    ended,
                    format!("failed to build tokio runtime: {e}"),
                );
            }
        };
        let response = rt.block_on(async {
            let client = reqwest::Client::builder()
                .timeout(Duration::from_secs(10))
                .build()
                .map_err(|e| format!("build client: {e}"))?;
            let req = match method.as_str() {
                "GET" => client.get(&url),
                "HEAD" => client.head(&url),
                "POST" => client.post(&url),
                "PUT" => client.put(&url),
                "DELETE" => client.delete(&url),
                "PATCH" => client.patch(&url),
                other => {
                    return Err(format!("unsupported method: {other}"));
                }
            };
            req.send().await.map_err(|e| format!("send: {e}"))
        });
        let ended = iso_now();
        match response {
            Ok(resp) => {
                let status = resp.status().as_u16() as i64;
                let expected = expect_status.unwrap_or(200);
                if status == expected {
                    super::StepRunTrace {
                        outcome: dec_graph::ontology::verification_result::StepOutcome::Pass,
                        started_at: started,
                        ended_at: ended,
                        stdout_excerpt: String::new(),
                        stderr_excerpt: String::new(),
                        exit_code: Some(status),
                        error_message: String::new(),
                        stop_on_fail: false,
                    }
                } else {
                    super::StepRunTrace {
                        outcome: dec_graph::ontology::verification_result::StepOutcome::Fail,
                        started_at: started,
                        ended_at: ended,
                        stdout_excerpt: String::new(),
                        stderr_excerpt: String::new(),
                        exit_code: Some(status),
                        error_message: format!("expected status {expected}, got {status}"),
                        stop_on_fail: false,
                    }
                }
            }
            Err(e) => StepRunTrace::unrunnable(started, ended, format!("http request failed: {e}")),
        }
    }
}
