//! `sparql-assertion` step handler (FT-098 §Phase 3.2).
//!
//! Loads `dec:target` (a Turtle/N-Quads file under `dec_workdir` or an
//! HTTP SPARQL endpoint), runs the query via oxigraph, and asserts the
//! row count matches `dec:expectRows`.

use oxigraph::io::RdfFormat;
use oxigraph::sparql::QueryResults;
use oxigraph::store::Store;

use crate::core::ontology::verification_graph::{StepFields, VerificationStep};

use super::super::context::RunContext;
use super::common::iso_now;
use super::{StepKindHandler, StepRunTrace};

/// `sparql-assertion` handler.
pub struct SparqlHandler;

impl StepKindHandler for SparqlHandler {
    fn run(&self, step: &VerificationStep, ctx: &mut RunContext) -> StepRunTrace {
        let started = iso_now();
        let StepFields::SparqlAssertion {
            target,
            query,
            expect_rows,
        } = &step.fields
        else {
            let ended = iso_now();
            return StepRunTrace::unrunnable(
                started,
                ended,
                "sparql-assertion handler received non-sparql fields".into(),
            );
        };
        let target = match ctx.substitute(target) {
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
        let query_text = match ctx.substitute(query) {
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
        if target.starts_with("http://") || target.starts_with("https://") {
            // HTTP SPARQL endpoints are out of scope for the initial
            // landing — surface unrunnable rather than fake a result.
            let ended = iso_now();
            return StepRunTrace::unrunnable(
                started,
                ended,
                "remote sparql-http endpoints not supported in this slice".into(),
            );
        }
        let path = ctx.resolve_path(&target);
        if !path.exists() {
            let ended = iso_now();
            return StepRunTrace::unrunnable(
                started,
                ended,
                format!("target missing: {p}", p = path.display()),
            );
        }
        let bytes = match std::fs::read(&path) {
            Ok(b) => b,
            Err(e) => {
                let ended = iso_now();
                return StepRunTrace::unrunnable(
                    started,
                    ended,
                    format!("could not load target {p}: {e}", p = path.display()),
                );
            }
        };
        let store = match Store::new() {
            Ok(s) => s,
            Err(e) => {
                let ended = iso_now();
                return StepRunTrace::unrunnable(
                    started,
                    ended,
                    format!("failed to open in-memory store: {e}"),
                );
            }
        };
        if store
            .load_from_reader(RdfFormat::Turtle, bytes.as_slice())
            .is_err()
        {
            // Try N-Quads.
            if store
                .load_from_reader(RdfFormat::NQuads, bytes.as_slice())
                .is_err()
            {
                let ended = iso_now();
                return StepRunTrace::unrunnable(
                    started,
                    ended,
                    format!(
                        "target {p} did not parse as Turtle or N-Quads",
                        p = path.display()
                    ),
                );
            }
        }
        let results = match store.query(query_text.as_str()) {
            Ok(r) => r,
            Err(e) => {
                let ended = iso_now();
                return StepRunTrace::unrunnable(
                    started,
                    ended,
                    format!("query parse / evaluate error: {e}"),
                );
            }
        };
        let row_count = count_rows(results);
        let ended = iso_now();
        let expected = expect_rows.unwrap_or(0);
        let pass = match expect_rows {
            Some(_) => row_count == expected,
            None => true, // No assertion ⇒ trivially passes.
        };
        if pass {
            StepRunTrace::pass(started, ended)
        } else {
            StepRunTrace::fail(
                started,
                ended,
                format!("expected {expected} rows, got {row_count}"),
            )
        }
    }
}

fn count_rows(results: QueryResults) -> i64 {
    match results {
        QueryResults::Solutions(sols) => {
            let mut n: i64 = 0;
            for sol in sols {
                // SELECT ?n WHERE { … } with a COUNT aggregate is one
                // bound row but yields the count as a literal. We
                // treat any aggregated COUNT() literal as the row
                // count; otherwise we count rows directly.
                if let Ok(sol) = sol {
                    if let Some(value) = sol.iter().next() {
                        if let oxigraph::model::Term::Literal(lit) = value.1 {
                            if let Ok(parsed) = lit.value().parse::<i64>() {
                                return parsed;
                            }
                        }
                    }
                    n += 1;
                }
            }
            n
        }
        QueryResults::Boolean(b) => i64::from(b),
        QueryResults::Graph(_) => 0,
    }
}
