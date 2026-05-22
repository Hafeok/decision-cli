//! In-memory `CoverageWaiver` shape + RDF + Turtle serialisation.
//!
//! ADR-031 / FT-047. The artifact is a small, additive PROV-O record;
//! we keep the serialisation surface flat (no rdf:List) so the SHACL
//! validator stays simple.

use std::fmt::Write;

use chrono::{DateTime, Utc};
use oxigraph::model::{GraphName, Literal, NamedNode, NamedNodeRef, Quad};
use thiserror::Error;

use crate::core::vocab::{
    coverage_waiver_class, dcterms_created, uncovered_at_waive, waiver_for, waiver_reason,
    was_attributed_to, IRI_DEC_WAIVER_PREFIX,
};

pub(super) const RDF_TYPE: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#type";

/// Failures surfaced by waiver I/O helpers (canonical Turtle emit).
#[derive(Debug, Error)]
pub enum WaiverIoError {
    /// Writer failure during canonical serialisation.
    #[error("turtle emit failure: {0}")]
    Emit(String),
}

/// In-memory `dec:CoverageWaiver` artifact (FT-047 §Outputs).
///
/// Identity is the `id` field (e.g. `CW-NNN`); the canonical IRI is
/// `https://decision-cli.dev/ns/waiver/<id>`. Persistence path on disk
/// is `.dec/verify/waivers/<id>.ttl`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoverageWaiver {
    /// Identifier — e.g. `CW-001`.
    pub id: String,
    /// IRI of the feature the waiver targets.
    pub waiver_for: NamedNode,
    /// Reason — must be ≥ 16 non-whitespace characters (ADR-031).
    pub reason: String,
    /// PROV-O actor that minted the waiver (often `urn:dec:agent:cli`).
    pub attributed_to: NamedNode,
    /// RFC3339 instant captured at gate-firing time.
    pub created: DateTime<Utc>,
    /// Snapshot of every uncovered TC at the moment the gate ran.
    pub uncovered_at_waive: Vec<NamedNode>,
}

impl CoverageWaiver {
    /// Canonical IRI for this waiver.
    #[must_use]
    pub fn iri(&self) -> NamedNode {
        NamedNode::new_unchecked(format!("{IRI_DEC_WAIVER_PREFIX}{id}", id = self.id))
    }

    /// On-disk filename — `<id>.ttl`.
    #[must_use]
    pub fn filename(&self) -> String {
        format!("{}.ttl", self.id)
    }

    /// Serialise to RDF quads in the supplied named graph.
    #[must_use]
    pub fn to_quads(&self, graph: NamedNodeRef<'_>) -> Vec<Quad> {
        let g: GraphName = graph.into_owned().into();
        let subject = self.iri();
        let mut quads = waiver_core_quads(&subject, self, &g);
        quads.extend(self.uncovered_at_waive_quads(&subject, &g));
        quads
    }

    fn uncovered_at_waive_quads(&self, subject: &NamedNode, g: &GraphName) -> Vec<Quad> {
        self.uncovered_at_waive
            .iter()
            .map(|tc| Quad::new(subject.clone(), uncovered_at_waive(), tc.clone(), g.clone()))
            .collect()
    }

    /// Render the waiver as canonical Turtle for the on-disk file.
    ///
    /// The output uses literal prefixes so the file is human-readable
    /// without an external prefix table.
    pub fn to_canonical_turtle(&self) -> Result<String, WaiverIoError> {
        let mut out = String::new();
        write_turtle_prefixes(&mut out)?;
        write_turtle_header(&mut out, self)?;
        write_turtle_fixed_predicates(&mut out, self)?;
        finalise_turtle_with_optional_uncovered(&mut out, &self.uncovered_at_waive)?;
        Ok(out)
    }
}

fn waiver_core_quads(subject: &NamedNode, w: &CoverageWaiver, g: &GraphName) -> Vec<Quad> {
    let rdf_type = NamedNodeRef::new_unchecked(RDF_TYPE);
    let cls = coverage_waiver_class();
    vec![
        Quad::new(subject.clone(), rdf_type, cls, g.clone()),
        Quad::new(
            subject.clone(),
            waiver_for(),
            w.waiver_for.clone(),
            g.clone(),
        ),
        Quad::new(
            subject.clone(),
            waiver_reason(),
            Literal::new_simple_literal(&w.reason),
            g.clone(),
        ),
        Quad::new(
            subject.clone(),
            was_attributed_to(),
            w.attributed_to.clone(),
            g.clone(),
        ),
        Quad::new(
            subject.clone(),
            dcterms_created(),
            Literal::new_simple_literal(w.created.to_rfc3339()),
            g.clone(),
        ),
    ]
}

fn write_turtle_prefixes(out: &mut String) -> Result<(), WaiverIoError> {
    writeln!(out, "@prefix dec: <https://decision-cli.dev/ns#> .")
        .map_err(|e| WaiverIoError::Emit(e.to_string()))?;
    writeln!(out, "@prefix prov: <http://www.w3.org/ns/prov#> .")
        .map_err(|e| WaiverIoError::Emit(e.to_string()))?;
    writeln!(out, "@prefix dcterms: <http://purl.org/dc/terms/> .")
        .map_err(|e| WaiverIoError::Emit(e.to_string()))?;
    writeln!(out).map_err(|e| WaiverIoError::Emit(e.to_string()))?;
    Ok(())
}

fn write_turtle_header(out: &mut String, w: &CoverageWaiver) -> Result<(), WaiverIoError> {
    writeln!(out, "<{}>", w.iri().as_str()).map_err(|e| WaiverIoError::Emit(e.to_string()))?;
    writeln!(out, "    a dec:CoverageWaiver ;").map_err(|e| WaiverIoError::Emit(e.to_string()))?;
    Ok(())
}

fn write_turtle_fixed_predicates(
    out: &mut String,
    w: &CoverageWaiver,
) -> Result<(), WaiverIoError> {
    writeln!(out, "    dec:waiverFor <{}> ;", w.waiver_for.as_str())
        .map_err(|e| WaiverIoError::Emit(e.to_string()))?;
    writeln!(
        out,
        "    dec:waiverReason {} ;",
        escape_turtle_literal(&w.reason)
    )
    .map_err(|e| WaiverIoError::Emit(e.to_string()))?;
    writeln!(
        out,
        "    prov:wasAttributedTo <{}> ;",
        w.attributed_to.as_str()
    )
    .map_err(|e| WaiverIoError::Emit(e.to_string()))?;
    writeln!(
        out,
        "    dcterms:created {} ;",
        escape_turtle_literal(&w.created.to_rfc3339())
    )
    .map_err(|e| WaiverIoError::Emit(e.to_string()))?;
    Ok(())
}

fn finalise_turtle_with_optional_uncovered(
    out: &mut String,
    uncovered: &[NamedNode],
) -> Result<(), WaiverIoError> {
    if uncovered.is_empty() {
        replace_last_semicolon_with_dot(out);
        return Ok(());
    }
    let last = uncovered.len() - 1;
    for (i, tc) in uncovered.iter().enumerate() {
        let term = if i == last { "." } else { ";" };
        writeln!(out, "    dec:uncoveredAtWaive <{}> {}", tc.as_str(), term)
            .map_err(|e| WaiverIoError::Emit(e.to_string()))?;
    }
    Ok(())
}

fn replace_last_semicolon_with_dot(s: &mut String) {
    if let Some(idx) = s.rfind(" ;\n") {
        // Replace " ;\n" with " .\n"
        s.replace_range(idx..idx + 3, " .\n");
    }
}

fn escape_turtle_literal(value: &str) -> String {
    // Escape Turtle short-string literal characters.
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for ch in value.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::vocab::waivers_named_graph;

    fn fixture() -> CoverageWaiver {
        CoverageWaiver {
            id: "CW-001".to_string(),
            waiver_for: NamedNode::new_unchecked("https://decision-cli.dev/ns/feature/FT-U"),
            reason: "Doc-only feature; verification is review-based per ADR-NNN".to_string(),
            attributed_to: NamedNode::new_unchecked("urn:dec:agent:cli"),
            created: chrono::DateTime::parse_from_rfc3339("2026-05-21T18:00:00Z")
                .expect("rfc3339")
                .with_timezone(&Utc),
            uncovered_at_waive: vec![NamedNode::new_unchecked(
                "https://decision-cli.dev/ns/tc/TC-T2",
            )],
        }
    }

    #[test]
    fn iri_uses_prefix_and_id() {
        let w = fixture();
        assert_eq!(
            w.iri().as_str(),
            "https://decision-cli.dev/ns/waiver/CW-001"
        );
    }

    #[test]
    fn to_quads_emits_required_triples() {
        let w = fixture();
        let g = waivers_named_graph();
        let quads = w.to_quads(g);
        assert!(quads.iter().any(|q| q.predicate.as_str() == RDF_TYPE));
        assert!(quads
            .iter()
            .any(|q| q.predicate.as_str() == crate::core::vocab::IRI_DEC_WAIVER_FOR));
        assert!(quads
            .iter()
            .any(|q| q.predicate.as_str() == crate::core::vocab::IRI_DEC_WAIVER_REASON));
        assert!(quads
            .iter()
            .any(|q| q.predicate.as_str() == crate::core::vocab::IRI_DEC_UNCOVERED_AT_WAIVE));
    }

    #[test]
    fn canonical_turtle_round_trips_minimal_shape() {
        let w = fixture();
        let ttl = w.to_canonical_turtle().expect("ok");
        assert!(ttl.contains("dec:CoverageWaiver"));
        assert!(ttl.contains("CW-001"));
        assert!(ttl.contains("FT-U"));
        assert!(ttl.contains("TC-T2"));
        assert!(ttl.contains("Doc-only feature"));
    }

    #[test]
    fn canonical_turtle_with_no_uncovered_ends_with_dot() {
        let mut w = fixture();
        w.uncovered_at_waive.clear();
        let ttl = w.to_canonical_turtle().expect("ok");
        let trimmed = ttl.trim_end();
        assert!(trimmed.ends_with(" ."), "ttl: {ttl}");
    }
}
