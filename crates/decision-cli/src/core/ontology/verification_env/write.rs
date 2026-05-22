//! Canonical Turtle writer for `dec:VerificationEnvironment` (ADR-028).
//!
//! Byte-deterministic by construction — TC-055 relies on this output
//! being identical for the same input.

use crate::core::vocab::IRI_DEC_ENV_PREFIX;

use super::types::VerificationEnvironment;

/// Serialise an environment to its canonical Turtle form.
///
/// The byte layout is fixed so seed reproducibility (TC-055) does not
/// depend on an external serialiser's whims. Order of properties is
/// fixed: `a`, `dec:envType`, `dec:safetyClass`, `dec:allowedOps`,
/// `dec:setup`, `dec:teardown`, `dec:endpoint`, `dec:fixtureSource`.
#[must_use]
pub fn to_canonical_turtle(env: &VerificationEnvironment) -> String {
    let mut out = String::new();
    write_env_header(&mut out, env);
    write_env_required_fields(&mut out, env);
    write_env_optional_fields(&mut out, env);
    finalise_turtle_statement(&mut out);
    out
}

fn write_env_header(out: &mut String, env: &VerificationEnvironment) {
    use std::fmt::Write;
    out.push_str("# decision-cli VerificationEnvironment artifact.\n");
    out.push_str("# Authored by FT-035 / ADR-028. On-disk Turtle is authoritative.\n\n");
    out.push_str("@prefix dec: <https://decision-cli.dev/ns#> .\n\n");
    let _ = writeln!(
        out,
        "<{prefix}{id}>",
        prefix = IRI_DEC_ENV_PREFIX,
        id = env.id
    );
    out.push_str("    a dec:VerificationEnvironment ;\n");
}

fn write_env_required_fields(out: &mut String, env: &VerificationEnvironment) {
    use std::fmt::Write;
    let _ = writeln!(
        out,
        "    dec:envType {value} ;",
        value = turtle_string(&env.env_type)
    );
    let _ = writeln!(
        out,
        "    dec:safetyClass {value} ;",
        value = turtle_string(env.safety_class.as_str())
    );
    let ops_list = format_allowed_ops_list(&env.allowed_ops);
    let _ = writeln!(out, "    dec:allowedOps {ops_list} ;");
}

fn write_env_optional_fields(out: &mut String, env: &VerificationEnvironment) {
    use std::fmt::Write;
    if let Some(s) = &env.setup {
        let _ = writeln!(out, "    dec:setup {value} ;", value = turtle_string(s));
    }
    if let Some(s) = &env.teardown {
        let _ = writeln!(out, "    dec:teardown {value} ;", value = turtle_string(s));
    }
    if let Some(s) = &env.endpoint {
        let _ = writeln!(out, "    dec:endpoint {value} ;", value = turtle_string(s));
    }
    if let Some(s) = &env.fixture_source {
        let _ = writeln!(
            out,
            "    dec:fixtureSource {value} ;",
            value = turtle_string(s)
        );
    }
}

fn finalise_turtle_statement(out: &mut String) {
    // Replace the final ";" with "." for valid Turtle.
    if out.ends_with(" ;\n") {
        out.truncate(out.len() - 3);
        out.push_str(" .\n");
    }
}

fn format_allowed_ops_list(ops: &[String]) -> String {
    if ops.is_empty() {
        return "()".to_string();
    }
    let parts: Vec<String> = ops.iter().map(|op| turtle_string(op)).collect();
    let mut out = String::from("( ");
    out.push_str(&parts.join(" "));
    out.push_str(" )");
    out
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
