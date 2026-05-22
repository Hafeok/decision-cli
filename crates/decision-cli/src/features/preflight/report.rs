//! Preflight report data shapes (FT-052).
//!
//! Types describing the three sections the report exposes
//! (`cross_cutting_gaps`, `domain_gaps`, `dep_availability`) plus the
//! summary status. The render path lives here too so the contracts
//! and their string form are colocated.

use std::path::PathBuf;

/// Outcome of a single `pm:dependsOn` link: the dependency's id and the
/// status the projection records for it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DependencyStatus {
    /// Feature id, e.g. `"FT-008"`.
    pub feature_id: String,
    /// Status string as projected (e.g. `"Complete"`, `"Planned"`).
    /// `None` when the dependency exists in the projection but no
    /// `pm:status` is recorded for it (we surface this as a gap).
    pub status: Option<String>,
}

impl DependencyStatus {
    /// Whether this dep counts as *available* for dispatch. The
    /// projection's status is compared case-insensitively; only
    /// `"complete"` is accepted as available.
    #[must_use]
    pub fn is_available(&self) -> bool {
        self.status
            .as_deref()
            .is_some_and(|s| s.eq_ignore_ascii_case("complete"))
    }
}

/// One row of the cross-cutting coverage table: which cross-cutting
/// ADR was checked and what the projection says about its link to the
/// target feature.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrossCuttingRow {
    /// ADR id, e.g. `"ADR-013"`.
    pub adr_id: String,
    /// `true` iff the projection records `feature pm:implementedBy adr`
    /// or `adr pm:appliesTo feature` (either direction counts as a link).
    pub linked: bool,
}

/// Structured preflight report sourced from the projection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreflightReport {
    /// Feature id this report is about.
    pub feature_id: String,
    /// Cross-cutting ADRs that the projection says are **not** linked
    /// to the target feature. Empty when every cross-cutting ADR is
    /// covered.
    pub cross_cutting_gaps: Vec<String>,
    /// Cross-cutting ADRs that the projection says **are** linked to
    /// the target feature. Reported alongside gaps so the caller can
    /// render a status table without re-querying.
    pub cross_cutting_linked: Vec<String>,
    /// Domain coverage gaps: each entry is a domain name listed on the
    /// target feature for which the projection records no ADR with the
    /// matching `pm:domain` triple. Returns an empty list when the
    /// projection does not carry domain information (TC-087's source-
    /// of-truth contract is independent of this section).
    pub domain_gaps: Vec<String>,
    /// One row per `pm:dependsOn` link recorded by the projection.
    pub dep_availability: Vec<DependencyStatus>,
    /// Path the projection was read from. Surfaces in the rendered
    /// report so an operator can verify the source.
    pub projection_source: PathBuf,
}

impl PreflightReport {
    /// `true` iff any dependency is not complete or any domain gap
    /// exists. Cross-cutting gaps are surfaced but do not block
    /// dispatch — matching the legacy `product preflight` semantics
    /// where acknowledgement counts as coverage.
    #[must_use]
    pub fn has_blocking_gaps(&self) -> bool {
        !self.domain_gaps.is_empty() || self.dep_availability.iter().any(|d| !d.is_available())
    }

    /// Render the report as human-readable text. Output is stable so
    /// `dec preflight` can be diffed against `product preflight` on the
    /// three contracted sets.
    #[must_use]
    pub fn render(&self) -> String {
        use std::fmt::Write;
        let mut out = String::new();
        let _ = writeln!(
            out,
            "Pre-flight analysis: {}\nSource: {}",
            self.feature_id,
            self.projection_source.display()
        );
        render_section(&mut out, "cross_cutting_gaps", &self.cross_cutting_gaps);
        render_section(&mut out, "domain_gaps", &self.domain_gaps);
        self.render_deps(&mut out);
        out.push_str("\nPre-flight: ");
        out.push_str(if self.has_blocking_gaps() {
            "BLOCKED\n"
        } else {
            "CLEAN\n"
        });
        out
    }

    fn render_deps(&self, out: &mut String) {
        use std::fmt::Write;
        out.push_str("\ndep_availability:\n");
        if self.dep_availability.is_empty() {
            out.push_str("  (none)\n");
            return;
        }
        for d in &self.dep_availability {
            let status = d.status.as_deref().unwrap_or("(unknown)");
            let marker = if d.is_available() { "OK" } else { "BLOCK" };
            let _ = writeln!(out, "  [{marker}] {} : {status}", d.feature_id);
        }
    }
}

fn render_section(out: &mut String, label: &str, items: &[String]) {
    use std::fmt::Write;
    let _ = writeln!(out, "\n{label}:");
    if items.is_empty() {
        out.push_str("  (none)\n");
        return;
    }
    for i in items {
        let _ = writeln!(out, "  - {i}");
    }
}
