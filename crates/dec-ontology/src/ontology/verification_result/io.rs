//! Turtle round-trip helpers for `VerificationGraphResult` (FT-097 / ADR-028).
//!
//! Deterministic canonical-form serialiser; the on-disk file is authoritative
//! per ADR-028 §Storage format.

use super::types::{VerificationGraphResult, VerificationStepTrace};

/// Serialise a result (and every trace + evidence projection) to
/// canonical Turtle form.
#[must_use]
pub fn to_canonical_turtle(result: &VerificationGraphResult) -> String {
    let mut out = String::new();
    out.push_str("# decision-cli VerificationGraphResult artifact.\n");
    out.push_str("# Authored by FT-097 / ADR-028. On-disk Turtle is authoritative.\n\n");
    out.push_str("@prefix dec: <https://decision-cli.dev/ns#> .\n");
    out.push_str("@prefix dcterms: <http://purl.org/dc/terms/> .\n");
    out.push_str("@prefix prov: <http://www.w3.org/ns/prov#> .\n");
    out.push_str("@prefix rdf: <http://www.w3.org/1999/02/22-rdf-syntax-ns#> .\n");
    out.push_str("@prefix xsd: <http://www.w3.org/2001/XMLSchema#> .\n\n");

    write_result_header(&mut out, result);
    for trace in &result.step_traces {
        out.push('\n');
        write_step_trace(&mut out, trace);
    }
    out
}

fn write_result_header(out: &mut String, result: &VerificationGraphResult) {
    use std::fmt::Write;
    let _ = writeln!(out, "<{}>", result.id.as_str());
    out.push_str("    a dec:VerificationGraphResult ;\n");
    let _ = writeln!(out, "    dec:resultOf <{}> ;", result.result_of);
    let _ = writeln!(
        out,
        "    dec:ranInEnvironment <{}> ;",
        result.ran_in_environment
    );
    let _ = writeln!(
        out,
        "    dec:verdict {} ;",
        turtle_string(result.verdict.as_str())
    );
    let _ = writeln!(
        out,
        "    dec:startedAt {} ;",
        turtle_datetime(&result.started_at)
    );
    let _ = writeln!(
        out,
        "    dec:endedAt {} ;",
        turtle_datetime(&result.ended_at)
    );
    if result.step_traces.is_empty() {
        out.push_str("    dec:stepTraces () ;\n");
    } else {
        out.push_str("    dec:stepTraces (");
        for trace in &result.step_traces {
            let _ = write!(out, " <{}>", trace.id.as_str());
        }
        out.push_str(" ) ;\n");
    }
    let _ = writeln!(
        out,
        "    dec:rationale {} ;",
        turtle_string(&result.rationale)
    );
    for projection in &result.evidence_for {
        out.push_str("    dec:evidenceFor [\n");
        out.push_str("        a dec:EvidenceProjection ;\n");
        let _ = writeln!(out, "        dec:tc <{}> ;", projection.tc);
        let _ = writeln!(
            out,
            "        dec:outcome {} ;",
            turtle_string(projection.outcome.as_str())
        );
        let _ = writeln!(out, "        dec:fromStep <{}>", projection.from_step);
        out.push_str("    ] ;\n");
    }
    let _ = writeln!(
        out,
        "    prov:wasGeneratedBy <{}> ;",
        result.was_generated_by
    );
    let _ = writeln!(
        out,
        "    prov:wasAttributedTo <{}> ;",
        result.was_attributed_to
    );
    let _ = writeln!(
        out,
        "    dcterms:created {} .",
        turtle_datetime(&result.created_at)
    );
}

fn write_step_trace(out: &mut String, trace: &VerificationStepTrace) {
    use std::fmt::Write;
    let _ = writeln!(out, "<{}>", trace.id);
    out.push_str("    a dec:VerificationStepTrace ;\n");
    let _ = writeln!(out, "    dec:tracesStep <{}> ;", trace.traces_step);
    let _ = writeln!(
        out,
        "    dec:outcome {} ;",
        turtle_string(trace.outcome.as_str())
    );
    let _ = writeln!(
        out,
        "    dec:startedAt {} ;",
        turtle_datetime(&trace.started_at)
    );
    let _ = writeln!(
        out,
        "    dec:endedAt {} ;",
        turtle_datetime(&trace.ended_at)
    );
    if let Some(code) = trace.exit_code {
        let _ = writeln!(out, "    dec:exitCode {code} ;");
    }
    let _ = writeln!(
        out,
        "    dec:stdoutExcerpt {} ;",
        turtle_string(&trace.stdout_excerpt)
    );
    let _ = writeln!(
        out,
        "    dec:stderrExcerpt {} ;",
        turtle_string(&trace.stderr_excerpt)
    );
    if !trace.error_message.is_empty() {
        let _ = writeln!(
            out,
            "    dec:errorMessage {} ;",
            turtle_string(&trace.error_message)
        );
    }
    let _ = writeln!(
        out,
        "    prov:wasGeneratedBy <{}> .",
        trace.was_generated_by
    );
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

fn turtle_datetime(value: &str) -> String {
    format!("\"{value}\"^^xsd:dateTime")
}
