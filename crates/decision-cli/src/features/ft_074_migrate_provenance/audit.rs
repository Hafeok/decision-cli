//! Three-class audit (FT-074 §Behaviour step 1).
//!
//! Classifies every artifact in the corpus into one of three buckets:
//!
//! * `Conformant` — already carries both mechanical and motivational blocks.
//! * `BackfillableMechanical` — has a motivational mapping via the FT-074
//!   mapping table but no mechanical block.
//! * `Orphan` — neither block, no informal edges to map.
//!
//! Idempotence (FT-074 §Behaviour step 7) is encoded by treating artifacts
//! with `:isMigrationBackfill true` on their attributed Session as already
//! migrated (i.e. they remain `Conformant`).

#![allow(missing_docs)]

use anyhow::Result;
use oxigraph::sparql::QueryResults;
use oxigraph::store::Store;
use serde::{Deserialize, Serialize};

use crate::core::vocab::{IRI_PROV_GENERATED_AT_TIME, IRI_PROV_WAS_GENERATED_BY};

use super::mapping::{IRI_DEC_ADR, IRI_DEC_DEPENDENCY, IRI_DEC_FEATURE, IRI_DEC_TC};

/// Slice-1 audit scope per FT-074 §Inputs. Includes Feature even though
/// the FT-074 mapping table has no informal-field row that maps a
/// Feature to a motivational predicate — Features must still be
/// classifiable so genuinely orphan Features surface as Feedback
/// (FT-074 §Behaviour step 2 explicitly states Features remain orphans
/// in the new vocabulary because the Feature→ADR direction isn't
/// motivational).
const AUDIT_TARGET_TYPES: &[&str] = &[IRI_DEC_FEATURE, IRI_DEC_ADR, IRI_DEC_TC, IRI_DEC_DEPENDENCY];

/// Motivational predicates each artifact type accepts per the FT-072
/// per-type shapes. The audit uses this table to recognise an existing
/// motivational edge regardless of whether the FT-074 mapping table
/// would produce one via backfill — a Feature carrying `:addresses` is
/// conformant even though the mapping table has no Feature row.
fn motivational_predicates_for_audit(rdf_type: &str) -> &'static [&'static str] {
    match rdf_type {
        // shapes/feature.ttl
        IRI_DEC_FEATURE => &[
            "https://decision-cli.dev/ns#addresses",
            "https://decision-cli.dev/ns#decomposesFrom",
            "https://decision-cli.dev/ns#originatedFrom",
            "https://decision-cli.dev/ns#respondsTo",
        ],
        // shapes/adr.ttl
        IRI_DEC_ADR => &[
            "https://decision-cli.dev/ns#addresses",
            "https://decision-cli.dev/ns#decidesFor",
            "https://decision-cli.dev/ns#supersedes",
        ],
        // shapes/tc.ttl
        IRI_DEC_TC => &["https://decision-cli.dev/ns#validates"],
        // shapes/dependency.ttl
        IRI_DEC_DEPENDENCY => &["https://decision-cli.dev/ns#requiredBy"],
        _ => &[],
    }
}

/// One entry in the audit report, suitable for JSON serialisation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditEntry {
    /// IRI of the artifact under audit.
    pub artifact: String,
    /// `rdf:type` literal of the artifact.
    pub rdf_type: String,
    /// Verdict for this artifact.
    pub verdict: AuditVerdict,
}

/// Three-class verdict per FT-074 §Behaviour step 1.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum AuditVerdict {
    /// Both blocks present — no migration needed.
    Conformant,
    /// Motivational present via informal field(s); mechanical missing.
    BackfillableMechanical {
        /// One or more (predicate, target) pairs that satisfy the
        /// type's motivational `sh:or` block.
        edges: Vec<EdgeMap>,
    },
    /// Neither block present and no informal edges to map.
    Orphan {
        /// Human-readable reasons (each cites a structural failure).
        reasons: Vec<String>,
    },
}

/// One concrete motivational edge that backfill will materialise.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EdgeMap {
    pub predicate: String,
    pub target: String,
}

/// Audit every typed artifact in `store`. Returns one entry per artifact
/// IRI whose `rdf:type` is in the slice-1 mapping (`Feature`, `ADR`, `TC`,
/// `Dependency`).
pub fn audit_store(store: &Store) -> Result<Vec<AuditEntry>> {
    let mut entries = Vec::new();
    for (artifact, rdf_type) in typed_artifacts(store)? {
        let verdict = classify(store, &artifact, &rdf_type)?;
        entries.push(AuditEntry {
            artifact,
            rdf_type,
            verdict,
        });
    }
    Ok(entries)
}

/// Return `(artifact_iri, rdf_type_iri)` rows for every artifact whose
/// type is named in the slice-1 mapping. Migration only touches these
/// four product-cli artifact classes (see FT-074 §Inputs).
fn typed_artifacts(store: &Store) -> Result<Vec<(String, String)>> {
    let mut out = Vec::new();
    let mut seen: Vec<String> = Vec::new();
    let type_clause = AUDIT_TARGET_TYPES
        .iter()
        .map(|t| format!("<{t}>"))
        .collect::<Vec<_>>()
        .join(", ");
    let sparql = format!(
        "SELECT DISTINCT ?s ?t WHERE {{ \
           {{ ?s a ?t }} UNION {{ GRAPH ?g {{ ?s a ?t }} }} \
           FILTER (?t IN ({type_clause})) \
         }}"
    );
    if let QueryResults::Solutions(sols) = store.query(sparql.as_str())? {
        for sol in sols.flatten() {
            let Some(oxigraph::model::Term::NamedNode(s)) = sol.get("s") else {
                continue;
            };
            let Some(oxigraph::model::Term::NamedNode(t)) = sol.get("t") else {
                continue;
            };
            if seen.iter().any(|x| x == s.as_str()) {
                continue;
            }
            seen.push(s.as_str().to_string());
            out.push((s.as_str().to_string(), t.as_str().to_string()));
        }
    }
    Ok(out)
}

fn classify(store: &Store, artifact: &str, rdf_type: &str) -> Result<AuditVerdict> {
    let has_mech = has_mechanical_block(store, artifact)?;
    let edges = collect_motivational_edges(store, artifact, rdf_type)?;
    let has_motiv = !edges.is_empty();

    if has_mech && has_motiv {
        return Ok(AuditVerdict::Conformant);
    }

    // Idempotence: if the artifact already has a `:isMigrationBackfill true`
    // session linked via wasGeneratedBy, treat it as conformant — no
    // re-backfill needed (FT-074 §Behaviour step 7).
    if is_already_migration_backfilled(store, artifact)? {
        return Ok(AuditVerdict::Conformant);
    }

    if has_motiv {
        // Motivational present (via informal mapped fields); mechanical missing.
        return Ok(AuditVerdict::BackfillableMechanical { edges });
    }

    let mut reasons = Vec::new();
    if !has_mech {
        reasons.push("missing mechanical provenance block (prov:wasGeneratedBy / wasAttributedTo / generatedAtTime)".to_string());
    }
    reasons.push(format!(
        "no informal-field mapping to motivational predicates available for type <{rdf_type}>"
    ));
    Ok(AuditVerdict::Orphan { reasons })
}

fn has_mechanical_block(store: &Store, artifact: &str) -> Result<bool> {
    // Mechanical conformance requires at least `prov:wasGeneratedBy`
    // (and a `prov:generatedAtTime`). The full FT-069 shape also
    // requires `prov:wasAttributedTo`; for audit purposes the
    // wasGeneratedBy is the load-bearing presence check.
    let sparql = format!(
        "ASK {{ \
           {{ <{a}> <{p1}> ?s . <{a}> <{p2}> ?t . }} \
           UNION \
           {{ GRAPH ?g {{ <{a}> <{p1}> ?s . <{a}> <{p2}> ?t . }} }} \
         }}",
        a = artifact,
        p1 = IRI_PROV_WAS_GENERATED_BY,
        p2 = IRI_PROV_GENERATED_AT_TIME
    );
    match store.query(sparql.as_str())? {
        QueryResults::Boolean(b) => Ok(b),
        _ => Ok(false),
    }
}

fn collect_motivational_edges(
    store: &Store,
    artifact: &str,
    rdf_type: &str,
) -> Result<Vec<EdgeMap>> {
    // The audit uses the per-type SHACL shape's motivational predicate
    // set (so artifacts carrying genuine motivational triples are
    // recognised regardless of whether the mapping table would backfill
    // them). The backfill logic uses `predicates_for_source_type`,
    // which is intentionally narrower.
    let preds = motivational_predicates_for_audit(rdf_type);
    let mut edges = Vec::new();
    for p in preds {
        let sparql = format!(
            "SELECT ?o WHERE {{ \
               {{ <{a}> <{p}> ?o }} UNION {{ GRAPH ?g {{ <{a}> <{p}> ?o }} }} \
             }}",
            a = artifact,
            p = p
        );
        if let QueryResults::Solutions(sols) = store.query(sparql.as_str())? {
            for sol in sols.flatten() {
                if let Some(oxigraph::model::Term::NamedNode(o)) = sol.get("o") {
                    edges.push(EdgeMap {
                        predicate: (*p).to_string(),
                        target: o.as_str().to_string(),
                    });
                }
            }
        }
    }
    Ok(edges)
}

fn is_already_migration_backfilled(store: &Store, artifact: &str) -> Result<bool> {
    let sparql = format!(
        "ASK {{ \
           {{ <{a}> <{wgb}> ?s . ?s <{flag}> ?v . FILTER(?v = true || str(?v) = \"true\") }} \
           UNION \
           {{ GRAPH ?g {{ <{a}> <{wgb}> ?s . ?s <{flag}> ?v . FILTER(?v = true || str(?v) = \"true\") }} }} \
         }}",
        a = artifact,
        wgb = IRI_PROV_WAS_GENERATED_BY,
        flag = "https://decision-cli.dev/ns#isMigrationBackfill",
    );
    match store.query(sparql.as_str())? {
        QueryResults::Boolean(b) => Ok(b),
        _ => Ok(false),
    }
}
