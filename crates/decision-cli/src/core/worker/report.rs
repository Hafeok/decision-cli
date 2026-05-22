//! Preflight report shape — assembly + text/JSON rendering.
//!
//! Consumed by `dec init` (advisory), `dec doctor` (authoritative), or
//! `dec implement` (for the diagnostic shown on the abort path).

use std::path::Path;

use super::manifest::{manifest_sha256_hex, WorkerEntry, MANIFEST};
use super::resolve::{resolve, Resolution, ResolveInputs};

/// One row in the report.
#[derive(Debug, Clone)]
pub struct WorkerRow {
    /// The role name (e.g. `code-writer`).
    pub role: String,
    /// OK / Missing / Inactive — slice-1's three terminal statuses.
    pub status: RoleStatus,
    /// Step in the chain that matched, when `status == Ok`.
    pub resolved_via: Option<&'static str>,
    /// Resolved argv when `status == Ok`.
    pub resolved_command: Vec<String>,
    /// Source-hint-derived suggestions when `status == Missing`.
    pub install_hints: Vec<String>,
    /// Probe-time stderr / stale-entry notes.
    pub diagnostics: Vec<String>,
    /// Manifest entry, when this role is in the manifest.
    pub entry: Option<&'static WorkerEntry>,
}

/// Status of a single role row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoleStatus {
    /// Resolution chain found a usable invocation.
    Ok,
    /// Required by the value stream but no invocation found.
    Missing,
    /// In the manifest but not referenced by the active value stream.
    Inactive,
}

impl RoleStatus {
    /// Lower-kebab form (TC-048 #3).
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::Missing => "missing",
            Self::Inactive => "inactive",
        }
    }
}

/// Full preflight report.
#[derive(Debug, Clone)]
pub struct WorkerReport {
    /// One row per manifest entry (inactive rows included).
    pub rows: Vec<WorkerRow>,
    /// Fingerprint of `manifest.toml` for audit / JSON output.
    pub manifest_sha256: String,
}

impl WorkerReport {
    /// True iff no row has `status == Missing`.
    #[must_use]
    pub fn is_all_ok(&self) -> bool {
        !self.rows.iter().any(|r| r.status == RoleStatus::Missing)
    }

    /// Count of rows by status.
    #[must_use]
    pub fn counts(&self) -> (usize, usize, usize) {
        let mut ok = 0;
        let mut missing = 0;
        let mut inactive = 0;
        for r in &self.rows {
            match r.status {
                RoleStatus::Ok => ok += 1,
                RoleStatus::Missing => missing += 1,
                RoleStatus::Inactive => inactive += 1,
            }
        }
        (ok, missing, inactive)
    }
}

/// Build a report against the given active-roles set (typically the
/// value stream's declared role set). `roles_filter`, if `Some`, restricts
/// the report to a single role (`dec doctor --role <role>`).
#[must_use]
pub fn build_report(
    active_roles: &[&str],
    workdir: Option<&Path>,
    override_command: Option<&str>,
    roles_filter: Option<&str>,
) -> WorkerReport {
    let mut rows = Vec::new();
    for entry in MANIFEST {
        if let Some(filter) = roles_filter {
            if filter != entry.role {
                continue;
            }
        }
        rows.push(row_for_entry(
            entry,
            active_roles,
            workdir,
            override_command,
        ));
    }
    WorkerReport {
        rows,
        manifest_sha256: manifest_sha256_hex(),
    }
}

fn row_for_entry(
    entry: &'static WorkerEntry,
    active_roles: &[&str],
    workdir: Option<&Path>,
    override_command: Option<&str>,
) -> WorkerRow {
    if !active_roles.contains(&entry.role) {
        return inactive_row(entry);
    }
    let res = resolve(
        entry,
        ResolveInputs {
            override_command,
            workdir,
        },
    );
    match res {
        Resolution::Resolved {
            kind,
            argv,
            diagnostics,
        } => resolved_row(entry, kind.as_str(), argv, diagnostics),
        Resolution::Missing { diagnostics } => missing_row(entry, diagnostics),
    }
}

fn inactive_row(entry: &'static WorkerEntry) -> WorkerRow {
    WorkerRow {
        role: entry.role.to_string(),
        status: RoleStatus::Inactive,
        resolved_via: None,
        resolved_command: Vec::new(),
        install_hints: Vec::new(),
        diagnostics: Vec::new(),
        entry: Some(entry),
    }
}

fn resolved_row(
    entry: &'static WorkerEntry,
    via: &'static str,
    argv: Vec<String>,
    diagnostics: Vec<String>,
) -> WorkerRow {
    WorkerRow {
        role: entry.role.to_string(),
        status: RoleStatus::Ok,
        resolved_via: Some(via),
        resolved_command: argv,
        install_hints: Vec::new(),
        diagnostics,
        entry: Some(entry),
    }
}

fn missing_row(entry: &'static WorkerEntry, diagnostics: Vec<String>) -> WorkerRow {
    WorkerRow {
        role: entry.role.to_string(),
        status: RoleStatus::Missing,
        resolved_via: None,
        resolved_command: Vec::new(),
        install_hints: install_hints_for(entry),
        diagnostics,
        entry: Some(entry),
    }
}

/// Suggestion list derived from `install_kind` + `source_hint`.
fn install_hints_for(entry: &WorkerEntry) -> Vec<String> {
    let mut hints = Vec::new();
    match entry.install_kind {
        "uv-tool" => {
            hints.push(format!("uv tool install {}", entry.source_hint));
            hints.push(format!(
                "uv tool install {}    # published package (when available)",
                entry.console_script
            ));
        }
        "pipx" => {
            hints.push(format!("pipx install {}", entry.source_hint));
        }
        "cargo-install" => {
            hints.push(format!("cargo install --path {}", entry.source_hint));
        }
        other => {
            hints.push(format!(
                "(install via {other}) source: {}",
                entry.source_hint
            ));
        }
    }
    hints.push(format!(
        "Or set {env_var} to an explicit invocation, e.g.:\n        export {env_var}=\"/path/to/.venv/bin/{bin} run-once\"",
        env_var = entry.env_var,
        bin = entry.console_script
    ));
    hints
}

/// Render the report in the fixed-width text shape from FT-016 §Outputs.
#[must_use]
pub fn format_report_text(report: &WorkerReport) -> String {
    let mut out = String::new();
    out.push_str("Worker preflight:\n");
    if report.rows.is_empty() {
        out.push_str("  (no roles)\n");
        return out;
    }
    for row in &report.rows {
        out.push_str(&format_row_line(row));
    }
    for row in &report.rows {
        if row.status == RoleStatus::Missing && !row.install_hints.is_empty() {
            out.push_str("\nTo install:\n");
            for h in &row.install_hints {
                out.push_str(&format!("    {h}\n"));
            }
        }
    }
    out
}

fn format_row_line(row: &WorkerRow) -> String {
    match row.status {
        RoleStatus::Ok => {
            let argv0 = row.resolved_command.first().map_or("", String::as_str);
            let via = row.resolved_via.unwrap_or("");
            format!(
                "  {role:<12} OK   {argv0} (resolved via {via})\n",
                role = row.role
            )
        }
        RoleStatus::Missing => format!(
            "  {role:<12} MISSING  no resolution found\n",
            role = row.role
        ),
        RoleStatus::Inactive => format!(
            "  {role:<12} —    role not active in current value stream\n",
            role = row.role
        ),
    }
}

/// Render the canonical JSON form (TC-048).
#[must_use]
pub fn format_report_json(report: &WorkerReport) -> String {
    let (ok, missing, inactive) = report.counts();
    let workers_arr: Vec<serde_json::Value> = report.rows.iter().map(row_to_json).collect();
    let summary = build_summary_json(ok, missing, inactive);
    let mut doc = serde_json::Map::new();
    doc.insert("workers".into(), serde_json::Value::Array(workers_arr));
    doc.insert("summary".into(), serde_json::Value::Object(summary));
    doc.insert(
        "manifest_sha256".into(),
        serde_json::Value::String(report.manifest_sha256.clone()),
    );
    serde_json::Value::Object(doc).to_string()
}

fn row_to_json(row: &WorkerRow) -> serde_json::Value {
    let mut obj = serde_json::Map::new();
    obj.insert("role".into(), serde_json::Value::String(row.role.clone()));
    obj.insert(
        "status".into(),
        serde_json::Value::String(row.status.as_str().into()),
    );
    obj.insert("resolved_via".into(), resolved_via_json(row.resolved_via));
    obj.insert(
        "resolved_command".into(),
        str_array_json(&row.resolved_command),
    );
    obj.insert("install_hints".into(), str_array_json(&row.install_hints));
    obj.insert("diagnostics".into(), str_array_json(&row.diagnostics));
    serde_json::Value::Object(obj)
}

fn resolved_via_json(via: Option<&'static str>) -> serde_json::Value {
    via.map_or(serde_json::Value::Null, |v| {
        serde_json::Value::String(v.into())
    })
}

fn str_array_json(items: &[String]) -> serde_json::Value {
    serde_json::Value::Array(
        items
            .iter()
            .map(|s| serde_json::Value::String(s.clone()))
            .collect(),
    )
}

fn build_summary_json(
    ok: usize,
    missing: usize,
    inactive: usize,
) -> serde_json::Map<String, serde_json::Value> {
    let mut summary = serde_json::Map::new();
    summary.insert("ok".into(), serde_json::Value::Number(ok.into()));
    summary.insert("missing".into(), serde_json::Value::Number(missing.into()));
    summary.insert(
        "inactive".into(),
        serde_json::Value::Number(inactive.into()),
    );
    summary
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inactive_row_when_role_not_in_active_set() {
        let report = build_report(&[], None, None, None);
        assert_eq!(report.rows.len(), 1);
        assert_eq!(report.rows[0].status, RoleStatus::Inactive);
        let text = format_report_text(&report);
        assert!(text.contains("Worker preflight:"));
        assert!(text.contains("role not active"));
    }

    #[test]
    fn role_filter_restricts_output() {
        let report = build_report(&["code-writer"], None, None, Some("code-writer"));
        assert_eq!(report.rows.len(), 1);
        assert_eq!(report.rows[0].role, "code-writer");
    }

    #[test]
    fn install_hints_mention_source_hint_and_env_var() {
        let report = build_report(&["code-writer"], None, None, None);
        if report.rows[0].status == RoleStatus::Missing {
            let hints = &report.rows[0].install_hints;
            assert!(hints.iter().any(|h| h.contains("./workers/code-writer")));
            assert!(hints.iter().any(|h| h.contains("CODE_WRITER_CMD")));
        }
    }

    #[test]
    fn json_shape_has_required_fields() {
        let report = build_report(&["code-writer"], None, None, None);
        let json = format_report_json(&report);
        let v: serde_json::Value = serde_json::from_str(&json).expect("parses");
        assert!(v.get("workers").is_some());
        assert!(v.get("summary").is_some());
        assert!(v.get("manifest_sha256").is_some());
    }
}
