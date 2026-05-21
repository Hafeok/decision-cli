//! Canonical Turtle serialiser for `VerificationGraph` (FT-036 / ADR-028).
//!
//! Deterministic byte layout: header, then each step in `dec:steps`
//! rdf:List order. `${name}` placeholders pass through unchanged.

use super::types::{StepFields, VerificationGraph, VerificationStep};

/// Serialise a graph (and every step) to canonical Turtle form. The byte
/// layout is fixed so round-trip is structural rather than format-dependent.
#[must_use]
pub fn to_canonical_turtle(graph: &VerificationGraph) -> String {
    let mut out = String::new();
    out.push_str("# decision-cli VerificationGraph artifact.\n");
    out.push_str("# Authored by FT-036 / ADR-028. On-disk Turtle is authoritative.\n\n");
    out.push_str("@prefix dec: <https://decision-cli.dev/ns#> .\n");
    out.push_str("@prefix rdf: <http://www.w3.org/1999/02/22-rdf-syntax-ns#> .\n\n");
    write_graph_header(&mut out, graph);
    for step in &graph.steps {
        out.push('\n');
        write_step(&mut out, step);
    }
    out
}

fn write_graph_header(out: &mut String, graph: &VerificationGraph) {
    use std::fmt::Write;
    let _ = writeln!(out, "<{}>", graph.id.as_str());
    out.push_str("    a dec:VerificationGraph ;\n");
    let _ = writeln!(out, "    dec:verifies <{}> ;", graph.verifies.0.as_str());
    let _ = writeln!(out, "    dec:environment <{}> ;", graph.environment.as_str());
    if graph.steps.is_empty() {
        out.push_str("    dec:steps () .\n");
        return;
    }
    out.push_str("    dec:steps (");
    for step in &graph.steps {
        let _ = write!(out, " <{}>", step.id.as_str());
    }
    out.push_str(" ) .\n");
}

fn write_step(out: &mut String, step: &VerificationStep) {
    use std::fmt::Write;
    let _ = writeln!(out, "<{}>", step.id.as_str());
    out.push_str("    a dec:VerificationStep ;\n");
    let _ = writeln!(
        out,
        "    dec:stepType {} ;",
        turtle_string(step.kind.as_str())
    );
    for tc in &step.provides_evidence_for {
        let _ = writeln!(out, "    dec:providesEvidenceFor <{}> ;", tc.as_str());
    }
    write_step_fields(out, &step.fields);
    if out.ends_with(" ;\n") {
        out.truncate(out.len() - 3);
        out.push_str(" .\n");
    }
}

fn write_step_fields(out: &mut String, fields: &StepFields) {
    match fields {
        StepFields::ShellCommand {
            command,
            expect_exit_code,
            capture_output,
        } => write_shell_fields(out, command, *expect_exit_code, *capture_output),
        StepFields::SparqlAssertion {
            target,
            query,
            expect_rows,
        } => write_sparql_fields(out, target, query, *expect_rows),
        StepFields::FileAssertion {
            path,
            expect_hash,
            expect_content,
        } => write_file_fields(out, path, expect_hash.as_deref(), expect_content.as_deref()),
        StepFields::HttpRequest {
            method,
            url,
            expect_status,
        } => write_http_fields(out, method, url, *expect_status),
        StepFields::WaitFor { condition, timeout } => {
            write_wait_for_fields(out, condition.as_str(), timeout)
        }
        StepFields::Capture { from_step, bind_as } => write_capture_fields(
            out,
            bind_as,
            from_step.as_ref().map(oxigraph::model::NamedNode::as_str),
        ),
    }
}

fn write_shell_fields(
    out: &mut String,
    command: &str,
    expect_exit_code: Option<i64>,
    capture_output: Option<bool>,
) {
    use std::fmt::Write;
    let _ = writeln!(out, "    dec:command {} ;", turtle_string(command));
    if let Some(c) = expect_exit_code {
        let _ = writeln!(out, "    dec:expectExitCode {c} ;");
    }
    if let Some(b) = capture_output {
        let _ = writeln!(out, "    dec:captureOutput {b} ;");
    }
}

fn write_sparql_fields(out: &mut String, target: &str, query: &str, expect_rows: Option<i64>) {
    use std::fmt::Write;
    let _ = writeln!(out, "    dec:target {} ;", turtle_string(target));
    let _ = writeln!(out, "    dec:query {} ;", turtle_string(query));
    if let Some(r) = expect_rows {
        let _ = writeln!(out, "    dec:expectRows {r} ;");
    }
}

fn write_file_fields(
    out: &mut String,
    path: &str,
    expect_hash: Option<&str>,
    expect_content: Option<&str>,
) {
    use std::fmt::Write;
    let _ = writeln!(out, "    dec:path {} ;", turtle_string(path));
    if let Some(h) = expect_hash {
        let _ = writeln!(out, "    dec:expectHash {} ;", turtle_string(h));
    }
    if let Some(c) = expect_content {
        let _ = writeln!(out, "    dec:expectContent {} ;", turtle_string(c));
    }
}

fn write_http_fields(out: &mut String, method: &str, url: &str, expect_status: Option<i64>) {
    use std::fmt::Write;
    let _ = writeln!(out, "    dec:method {} ;", turtle_string(method));
    let _ = writeln!(out, "    dec:url {} ;", turtle_string(url));
    if let Some(s) = expect_status {
        let _ = writeln!(out, "    dec:expectStatus {s} ;");
    }
}

fn write_wait_for_fields(out: &mut String, condition: &str, timeout: &str) {
    use std::fmt::Write;
    let _ = writeln!(out, "    dec:condition <{condition}> ;");
    let _ = writeln!(out, "    dec:timeout {} ;", turtle_string(timeout));
}

fn write_capture_fields(out: &mut String, bind_as: &str, from_step: Option<&str>) {
    use std::fmt::Write;
    let _ = writeln!(out, "    dec:bindAs {} ;", turtle_string(bind_as));
    if let Some(node) = from_step {
        let _ = writeln!(out, "    dec:fromStep <{node}> ;");
    }
}

fn turtle_string(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for ch in value.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                use std::fmt::Write;
                let _ = write!(out, "\\u{:04X}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}
