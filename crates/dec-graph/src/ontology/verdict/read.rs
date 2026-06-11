//! Read API for `VerificationVerdict` artifacts (FT-020 §Behaviour).

use oxigraph::model::{NamedNode, Quad, Term};
use oxigraph::sparql::QueryResults;
use oxigraph::store::Store;
use thiserror::Error;

use dec_ontology::vocab::{
    IRI_DEC_AMENDMENT_GUIDANCE, IRI_DEC_IN_STREAM, IRI_DEC_RATIONALE, IRI_DEC_VERDICT,
    IRI_DEC_VIOLATES,
};

use super::shacl::{iri_values, literal_values};
use super::types::{Verdict, VerdictArtifact, PROV_USED, PROV_WAS_GENERATED_BY};

/// Errors produced by [`list_verdicts_for_dispatch`] / [`latest_verdict_for_dispatch`].
#[derive(Debug, Error)]
pub enum ReadError {
    /// Store contains a verdict artifact whose RDF shape contradicts the schema.
    #[error("malformed verdict <{iri}>: {detail}")]
    MalformedVerdict {
        /// IRI of the offending verdict subject.
        iri: String,
        /// Why the shape failed.
        detail: String,
    },
    /// Oxigraph store-level failure (read transactions, persistence).
    #[error("oxigraph store error: {0}")]
    Store(String),
}

/// Return every `VerificationVerdict` whose paired `InterpretationSession`
/// belongs to the dispatch identified by `dispatch_iri`. Order is by
/// `prov:endedAtTime` (lexical ascending). A missing dispatch returns
/// an empty `Vec` — not an error (FT-020 §Error handling).
pub fn list_verdicts_for_dispatch(
    store: &Store,
    dispatch_iri: &str,
) -> Result<Vec<VerdictArtifact>, ReadError> {
    let verdict_iris = read_verdict_iris_for_dispatch(store, dispatch_iri)?;
    let mut out: Vec<VerdictArtifact> = Vec::with_capacity(verdict_iris.len());
    for iri in verdict_iris {
        out.push(read_verdict_by_iri(store, &iri)?);
    }
    Ok(out)
}

/// Return the most recently produced verdict for a dispatch, or `None`
/// if the dispatch has none.
pub fn latest_verdict_for_dispatch(
    store: &Store,
    dispatch_iri: &str,
) -> Result<Option<VerdictArtifact>, ReadError> {
    let mut verdicts = list_verdicts_for_dispatch(store, dispatch_iri)?;
    Ok(verdicts.pop())
}

fn read_verdict_iris_for_dispatch(
    store: &Store,
    dispatch_iri: &str,
) -> Result<Vec<String>, ReadError> {
    let q = build_dispatch_query(dispatch_iri);
    let QueryResults::Solutions(sols) = store
        .query(q.as_str())
        .map_err(|e| ReadError::Store(e.to_string()))?
    else {
        return Ok(Vec::new());
    };
    let mut out: Vec<String> = Vec::new();
    for sol in sols {
        let sol = sol.map_err(|e| ReadError::Store(e.to_string()))?;
        if let Some(Term::NamedNode(n)) = sol.get("v") {
            let iri = n.as_str().to_string();
            if !out.contains(&iri) {
                out.push(iri);
            }
        }
    }
    Ok(out)
}

fn build_dispatch_query(dispatch_iri: &str) -> String {
    format!(
        "PREFIX dec:  <https://decision-cli.dev/ns#> \
         PREFIX prov: <http://www.w3.org/ns/prov#> \
         SELECT DISTINCT ?v ?ended WHERE {{ \
           {{ ?v a dec:VerificationVerdict ; \
                  prov:wasGeneratedBy ?s . \
              ?s dec:dispatch <{dispatch_iri}> . \
              OPTIONAL {{ ?s prov:endedAtTime ?ended }} }} \
           UNION \
           {{ GRAPH ?g {{ ?v a dec:VerificationVerdict ; \
                          prov:wasGeneratedBy ?s . \
                          ?s dec:dispatch <{dispatch_iri}> . \
                          OPTIONAL {{ ?s prov:endedAtTime ?ended }} }} }} \
         }} ORDER BY ASC(?ended)"
    )
}

fn read_verdict_by_iri(store: &Store, iri: &str) -> Result<VerdictArtifact, ReadError> {
    let quads = collect_verdict_quads(store, iri)?;
    parse_verdict_from_quads(iri, &quads)
}

fn collect_verdict_quads(store: &Store, iri: &str) -> Result<Vec<Quad>, ReadError> {
    let mut out: Vec<Quad> = Vec::new();
    let s = NamedNode::new(iri).map_err(|e| ReadError::Store(e.to_string()))?;
    for quad in store.quads_for_pattern(
        Some(oxigraph::model::Subject::NamedNode(s.clone()).as_ref()),
        None,
        None,
        None,
    ) {
        let q = quad.map_err(|e| ReadError::Store(e.to_string()))?;
        out.push(q);
    }
    Ok(out)
}

fn parse_verdict_from_quads(iri: &str, quads: &[Quad]) -> Result<VerdictArtifact, ReadError> {
    let subject = NamedNode::new(iri).map_err(|e| ReadError::Store(e.to_string()))?;
    let verdict = parse_verdict_field(iri, quads, &subject)?;
    let rationale_value =
        take_one_literal(iri, quads, &subject, IRI_DEC_RATIONALE, "dec:rationale")?;
    let amendment = take_optional_literal(iri, quads, &subject, IRI_DEC_AMENDMENT_GUIDANCE)?;
    let refs = parse_verdict_references(iri, quads, &subject)?;
    Ok(VerdictArtifact {
        iri: subject,
        verdict,
        rationale: rationale_value,
        violates: refs.violates,
        amendment_guidance: amendment,
        generated_by: refs.generated_by,
        used: refs.used,
        in_stream: refs.in_stream,
    })
}

struct VerdictReferences {
    generated_by: NamedNode,
    in_stream: NamedNode,
    used: Vec<NamedNode>,
    violates: Vec<NamedNode>,
}

fn parse_verdict_references(
    iri: &str,
    quads: &[Quad],
    subject: &NamedNode,
) -> Result<VerdictReferences, ReadError> {
    let generated_by = take_one_iri(
        iri,
        quads,
        subject,
        PROV_WAS_GENERATED_BY,
        "prov:wasGeneratedBy",
    )?;
    let in_stream = take_one_iri(iri, quads, subject, IRI_DEC_IN_STREAM, "dec:inStream")?;
    let used = collect_iris(quads, subject, PROV_USED);
    let violates = collect_iris(quads, subject, IRI_DEC_VIOLATES);
    if used.is_empty() {
        return Err(ReadError::MalformedVerdict {
            iri: iri.to_string(),
            detail: "no prov:used references".to_string(),
        });
    }
    Ok(VerdictReferences {
        generated_by,
        in_stream,
        used,
        violates,
    })
}

fn parse_verdict_field(
    iri: &str,
    quads: &[Quad],
    subject: &NamedNode,
) -> Result<Verdict, ReadError> {
    let raw = take_one_literal(iri, quads, subject, IRI_DEC_VERDICT, "dec:verdict")?;
    Verdict::parse(&raw).ok_or_else(|| ReadError::MalformedVerdict {
        iri: iri.to_string(),
        detail: format!("unknown dec:verdict value {raw:?}"),
    })
}

fn take_one_literal(
    iri: &str,
    quads: &[Quad],
    subject: &NamedNode,
    predicate: &str,
    label: &str,
) -> Result<String, ReadError> {
    let mut values = literal_values(quads, subject, predicate);
    match values.len() {
        0 => Err(ReadError::MalformedVerdict {
            iri: iri.to_string(),
            detail: format!("missing required {label}"),
        }),
        1 => Ok(values.remove(0)),
        n => Err(ReadError::MalformedVerdict {
            iri: iri.to_string(),
            detail: format!("expected exactly one {label}, found {n}"),
        }),
    }
}

fn take_optional_literal(
    iri: &str,
    quads: &[Quad],
    subject: &NamedNode,
    predicate: &str,
) -> Result<Option<String>, ReadError> {
    let mut values = literal_values(quads, subject, predicate);
    match values.len() {
        0 => Ok(None),
        1 => Ok(Some(values.remove(0))),
        n => Err(ReadError::MalformedVerdict {
            iri: iri.to_string(),
            detail: format!("expected at most one {predicate}, found {n}"),
        }),
    }
}

fn take_one_iri(
    iri: &str,
    quads: &[Quad],
    subject: &NamedNode,
    predicate: &str,
    label: &str,
) -> Result<NamedNode, ReadError> {
    let values = iri_values(quads, subject, predicate);
    match values.len() {
        0 => Err(ReadError::MalformedVerdict {
            iri: iri.to_string(),
            detail: format!("missing required {label}"),
        }),
        1 => NamedNode::new(&values[0]).map_err(|e| ReadError::MalformedVerdict {
            iri: iri.to_string(),
            detail: format!("invalid IRI for {label}: {e}"),
        }),
        n => Err(ReadError::MalformedVerdict {
            iri: iri.to_string(),
            detail: format!("expected exactly one {label}, found {n}"),
        }),
    }
}

fn collect_iris(quads: &[Quad], subject: &NamedNode, predicate: &str) -> Vec<NamedNode> {
    iri_values(quads, subject, predicate)
        .into_iter()
        .filter_map(|s| NamedNode::new(&s).ok())
        .collect()
}
