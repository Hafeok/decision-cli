//! SPARQL read API for `dec:WorkerImage` artifacts (FT-086 §Behaviour).
//!
//! Provides three discovery primitives the orchestration catalog needs:
//!
//! - [`query_by_id`] — fetch by id, returning every version.
//! - [`query_by_capability_tag`] — list images claiming a capability tag.
//! - [`query_by_eligibility_status`] — list images at a given lifecycle
//!   status (e.g. `qualified`).
//!
//! All queries scan both the default graph and every named graph so
//! callers can use whichever projection convention they prefer (the
//! `dec:graph/worker-image` named graph or a per-instance store).

use oxigraph::model::{NamedNode, Quad, Term};
use oxigraph::sparql::QueryResults;
use oxigraph::store::Store;
use thiserror::Error;

use crate::core::vocab::{
    IRI_DEC_BUILD_RUN_URL, IRI_DEC_CAPABILITY_TAG, IRI_DEC_COMPATIBLE_ROLE,
    IRI_DEC_CONFORMANCE_AUDIT, IRI_DEC_ELIGIBILITY_STATUS, IRI_DEC_REGISTRY_REF, IRI_DEC_SBOM_REF,
    IRI_DEC_SIGNED_BY_ISSUER, IRI_DEC_SIGNED_BY_SUBJECT, IRI_DEC_SOURCE_COMMIT_HASH,
    IRI_DEC_SOURCE_REPO_URI, IRI_DEC_WORKER_IMAGE, IRI_DEC_WORKER_IMAGE_ID,
    IRI_DEC_WORKER_IMAGE_NAME, IRI_DEC_WORKER_IMAGE_VERSION,
};

use super::types::{EligibilityStatus, WorkerImage};

/// Errors produced by the WorkerImage read API.
#[derive(Debug, Error)]
pub enum WorkerImageReadError {
    /// SPARQL query against the store failed.
    #[error("oxigraph store error: {0}")]
    Store(String),
    /// Loaded WorkerImage does not match the schema.
    #[error("malformed worker image <{iri}>: {detail}")]
    Malformed {
        /// IRI of the offending subject.
        iri: String,
        /// Why parsing failed.
        detail: String,
    },
}

/// Return every `dec:WorkerImage` whose `dec:worker_image_id` matches `id`.
/// Multiple versions of the same image return multiple rows; order is
/// `(version ascending)`.
pub fn query_by_id(store: &Store, id: &str) -> Result<Vec<WorkerImage>, WorkerImageReadError> {
    let iris = find_iris_by_predicate_literal(store, IRI_DEC_WORKER_IMAGE_ID, id)?;
    let mut out = load_many(store, &iris)?;
    out.sort_by(|a, b| a.version.cmp(&b.version));
    Ok(out)
}

/// Return every `dec:WorkerImage` claiming `tag` as a `dec:capability_tag`.
/// Result order is `(id ascending, version ascending)`.
pub fn query_by_capability_tag(
    store: &Store,
    tag: &str,
) -> Result<Vec<WorkerImage>, WorkerImageReadError> {
    let iris = find_iris_by_predicate_literal(store, IRI_DEC_CAPABILITY_TAG, tag)?;
    let mut out = load_many(store, &iris)?;
    out.sort_by(|a, b| a.id.cmp(&b.id).then_with(|| a.version.cmp(&b.version)));
    Ok(out)
}

/// Return every `dec:WorkerImage` whose `dec:eligibility_status` matches.
/// Result order is `(id ascending, version ascending)`.
pub fn query_by_eligibility_status(
    store: &Store,
    status: EligibilityStatus,
) -> Result<Vec<WorkerImage>, WorkerImageReadError> {
    let iris = find_iris_by_predicate_literal(store, IRI_DEC_ELIGIBILITY_STATUS, status.as_str())?;
    let mut out = load_many(store, &iris)?;
    out.sort_by(|a, b| a.id.cmp(&b.id).then_with(|| a.version.cmp(&b.version)));
    Ok(out)
}

fn find_iris_by_predicate_literal(
    store: &Store,
    predicate: &str,
    value: &str,
) -> Result<Vec<NamedNode>, WorkerImageReadError> {
    let q = format!(
        "PREFIX dec: <https://decision-cli.dev/ns#> \
         SELECT DISTINCT ?w WHERE {{ \
           {{ ?w a dec:WorkerImage ; <{predicate}> {value_lit} . }} \
           UNION \
           {{ GRAPH ?g {{ ?w a dec:WorkerImage ; <{predicate}> {value_lit} }} }} \
         }}",
        value_lit = format_sparql_string(value),
    );
    let QueryResults::Solutions(sols) = store
        .query(q.as_str())
        .map_err(|e| WorkerImageReadError::Store(e.to_string()))?
    else {
        return Ok(Vec::new());
    };
    let mut out: Vec<NamedNode> = Vec::new();
    for sol in sols {
        let sol = sol.map_err(|e| WorkerImageReadError::Store(e.to_string()))?;
        if let Some(Term::NamedNode(n)) = sol.get("w") {
            if !out.iter().any(|m| m == n) {
                out.push(n.clone());
            }
        }
    }
    Ok(out)
}

fn load_many(store: &Store, iris: &[NamedNode]) -> Result<Vec<WorkerImage>, WorkerImageReadError> {
    let mut out = Vec::with_capacity(iris.len());
    for iri in iris {
        out.push(load_one(store, iri)?);
    }
    Ok(out)
}

fn load_one(store: &Store, iri: &NamedNode) -> Result<WorkerImage, WorkerImageReadError> {
    let quads = collect_quads(store, iri)?;
    parse_worker_image(iri.as_str(), &quads)
}

fn collect_quads(store: &Store, iri: &NamedNode) -> Result<Vec<Quad>, WorkerImageReadError> {
    let mut out: Vec<Quad> = Vec::new();
    let subj = oxigraph::model::Subject::NamedNode(iri.clone());
    for quad in store.quads_for_pattern(Some(subj.as_ref()), None, None, None) {
        let q = quad.map_err(|e| WorkerImageReadError::Store(e.to_string()))?;
        out.push(q);
    }
    Ok(out)
}

fn parse_worker_image(iri: &str, quads: &[Quad]) -> Result<WorkerImage, WorkerImageReadError> {
    let subject = NamedNode::new(iri).map_err(|e| WorkerImageReadError::Store(e.to_string()))?;
    require_type(&subject, quads)?;
    let (id, name, version, registry_ref) = parse_image_identity(iri, &subject, quads)?;
    let (signed_by_subject, signed_by_issuer, sbom_ref) =
        parse_image_signing(iri, &subject, quads)?;
    let (source_repo_uri, source_commit_hash, build_run_url) =
        parse_image_source(iri, &subject, quads)?;
    let eligibility_status = parse_eligibility(iri, &subject, quads)?;
    let capability_tags = take_all_str(quads, &subject, IRI_DEC_CAPABILITY_TAG);
    let compatible_roles = take_all_iri(quads, &subject, IRI_DEC_COMPATIBLE_ROLE);
    let conformance_audits = take_all_iri(quads, &subject, IRI_DEC_CONFORMANCE_AUDIT);
    Ok(WorkerImage {
        id,
        name,
        version,
        registry_ref,
        capability_tags,
        compatible_roles,
        signed_by_subject,
        signed_by_issuer,
        sbom_ref,
        conformance_audits,
        eligibility_status,
        source_repo_uri,
        source_commit_hash,
        build_run_url,
    })
}

fn parse_image_identity(
    iri: &str,
    subject: &NamedNode,
    quads: &[Quad],
) -> Result<(String, String, String, String), WorkerImageReadError> {
    let id = take_one_str(
        iri,
        quads,
        subject,
        IRI_DEC_WORKER_IMAGE_ID,
        "dec:worker_image_id",
    )?;
    let name = take_one_str(
        iri,
        quads,
        subject,
        IRI_DEC_WORKER_IMAGE_NAME,
        "dec:worker_image_name",
    )?;
    let version = take_one_str(
        iri,
        quads,
        subject,
        IRI_DEC_WORKER_IMAGE_VERSION,
        "dec:worker_image_version",
    )?;
    let registry_ref = take_one_str(
        iri,
        quads,
        subject,
        IRI_DEC_REGISTRY_REF,
        "dec:registry_ref",
    )?;
    Ok((id, name, version, registry_ref))
}

fn parse_image_signing(
    iri: &str,
    subject: &NamedNode,
    quads: &[Quad],
) -> Result<(String, String, String), WorkerImageReadError> {
    let signed_by_subject = take_one_str(
        iri,
        quads,
        subject,
        IRI_DEC_SIGNED_BY_SUBJECT,
        "dec:signed_by_subject",
    )?;
    let signed_by_issuer = take_one_str(
        iri,
        quads,
        subject,
        IRI_DEC_SIGNED_BY_ISSUER,
        "dec:signed_by_issuer",
    )?;
    let sbom_ref = take_one_str(iri, quads, subject, IRI_DEC_SBOM_REF, "dec:sbom_ref")?;
    Ok((signed_by_subject, signed_by_issuer, sbom_ref))
}

fn parse_image_source(
    iri: &str,
    subject: &NamedNode,
    quads: &[Quad],
) -> Result<(String, String, String), WorkerImageReadError> {
    let source_repo_uri = take_one_str(
        iri,
        quads,
        subject,
        IRI_DEC_SOURCE_REPO_URI,
        "dec:source_repo_uri",
    )?;
    let source_commit_hash = take_one_str(
        iri,
        quads,
        subject,
        IRI_DEC_SOURCE_COMMIT_HASH,
        "dec:source_commit_hash",
    )?;
    let build_run_url = take_one_str(
        iri,
        quads,
        subject,
        IRI_DEC_BUILD_RUN_URL,
        "dec:build_run_url",
    )?;
    Ok((source_repo_uri, source_commit_hash, build_run_url))
}

fn parse_eligibility(
    iri: &str,
    subject: &NamedNode,
    quads: &[Quad],
) -> Result<EligibilityStatus, WorkerImageReadError> {
    let status_raw = take_one_str(
        iri,
        quads,
        subject,
        IRI_DEC_ELIGIBILITY_STATUS,
        "dec:eligibility_status",
    )?;
    EligibilityStatus::try_from_str(&status_raw).ok_or_else(|| WorkerImageReadError::Malformed {
        iri: iri.to_string(),
        detail: format!("unknown dec:eligibility_status value {status_raw:?}"),
    })
}

fn require_type(subject: &NamedNode, quads: &[Quad]) -> Result<(), WorkerImageReadError> {
    let has_type = quads.iter().any(|q| {
        q.predicate.as_str() == "http://www.w3.org/1999/02/22-rdf-syntax-ns#type"
            && matches!(&q.object, Term::NamedNode(n) if n.as_str() == IRI_DEC_WORKER_IMAGE)
            && matches!(&q.subject, oxigraph::model::Subject::NamedNode(s) if s == subject)
    });
    if has_type {
        Ok(())
    } else {
        Err(WorkerImageReadError::Malformed {
            iri: subject.as_str().to_string(),
            detail: "missing rdf:type dec:WorkerImage".to_string(),
        })
    }
}

fn take_one_str(
    iri: &str,
    quads: &[Quad],
    subject: &NamedNode,
    predicate: &str,
    label: &str,
) -> Result<String, WorkerImageReadError> {
    let values: Vec<String> = quads
        .iter()
        .filter(|q| q.predicate.as_str() == predicate)
        .filter(|q| matches!(&q.subject, oxigraph::model::Subject::NamedNode(s) if s == subject))
        .filter_map(|q| match &q.object {
            Term::Literal(lit) => Some(lit.value().to_string()),
            _ => None,
        })
        .collect();
    match values.len() {
        0 => Err(WorkerImageReadError::Malformed {
            iri: iri.to_string(),
            detail: format!("missing required {label}"),
        }),
        1 => Ok(values.into_iter().next().expect("len() == 1")),
        n => Err(WorkerImageReadError::Malformed {
            iri: iri.to_string(),
            detail: format!("expected exactly one {label}, found {n}"),
        }),
    }
}

fn take_all_str(quads: &[Quad], subject: &NamedNode, predicate: &str) -> Vec<String> {
    let mut out: Vec<String> = quads
        .iter()
        .filter(|q| q.predicate.as_str() == predicate)
        .filter(|q| matches!(&q.subject, oxigraph::model::Subject::NamedNode(s) if s == subject))
        .filter_map(|q| match &q.object {
            Term::Literal(lit) => Some(lit.value().to_string()),
            _ => None,
        })
        .collect();
    out.sort();
    out.dedup();
    out
}

fn take_all_iri(quads: &[Quad], subject: &NamedNode, predicate: &str) -> Vec<NamedNode> {
    let mut out: Vec<NamedNode> = quads
        .iter()
        .filter(|q| q.predicate.as_str() == predicate)
        .filter(|q| matches!(&q.subject, oxigraph::model::Subject::NamedNode(s) if s == subject))
        .filter_map(|q| match &q.object {
            Term::NamedNode(n) => Some(n.clone()),
            _ => None,
        })
        .collect();
    out.sort_by(|a, b| a.as_str().cmp(b.as_str()));
    out.dedup();
    out
}

fn format_sparql_string(value: &str) -> String {
    let escaped = value.replace('\\', "\\\\").replace('"', "\\\"");
    format!("\"{escaped}\"")
}
