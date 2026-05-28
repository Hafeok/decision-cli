//! Persistence helpers: copy triples, session-quads builder, subscription
//! seeding, hashing.

use std::fs;
use std::path::Path;

use oxigraph::io::RdfFormat;
use oxigraph::model::{GraphName, GraphNameRef, Literal, NamedNode, NamedNodeRef, Quad};
use oxigraph::store::Store;
use sha2::{Digest, Sha256};

use super::vocab::{
    DEC_DEFINITION_FORM, DEC_DEFINITION_HASH, DEC_DEFINITION_SOURCE, DEC_ONTOLOGY_VERSION,
    DEC_SESSION_CLASS, PROV_ACTIVITY, PROV_AT_TIME, PROV_DERIVED_FROM, RDF_TYPE,
};
use super::InitError;

pub(super) fn copy_triples_default(
    src: &Store,
    dest: &Store,
    src_graph: &NamedNode,
    dest_graph: &GraphName,
) -> Result<(), InitError> {
    let g_ref = GraphNameRef::NamedNode(src_graph.as_ref());
    let mut to_insert: Vec<Quad> = Vec::new();
    for q in src.quads_for_pattern(None, None, None, Some(g_ref)) {
        let q = q.map_err(|e| InitError::Internal(e.to_string()))?;
        let nq = Quad::new(
            q.subject.clone(),
            q.predicate.clone(),
            q.object.clone(),
            dest_graph.clone(),
        );
        to_insert.push(nq);
    }
    dest.transaction(|mut tx| {
        for q in &to_insert {
            tx.insert(q.as_ref())?;
        }
        Ok::<_, oxigraph::store::StorageError>(())
    })
    .map_err(|e| InitError::Internal(e.to_string()))?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(super) fn build_session_quads(
    session_iri: &NamedNode,
    graph: &GraphName,
    source_label: &str,
    definition_hash: &str,
    ontology_version: &str,
    form: &str,
    started_at: &str,
) -> Vec<Quad> {
    let g: GraphName = graph.clone();
    let rdf_type = NamedNodeRef::new_unchecked(RDF_TYPE);
    let session_class = NamedNodeRef::new_unchecked(DEC_SESSION_CLASS);
    let activity = NamedNodeRef::new_unchecked(PROV_ACTIVITY);
    let derived = NamedNodeRef::new_unchecked(PROV_DERIVED_FROM);
    let at_time = NamedNodeRef::new_unchecked(PROV_AT_TIME);
    let p_source = NamedNodeRef::new_unchecked(DEC_DEFINITION_SOURCE);
    let p_hash = NamedNodeRef::new_unchecked(DEC_DEFINITION_HASH);
    let p_ver = NamedNodeRef::new_unchecked(DEC_ONTOLOGY_VERSION);
    let p_form = NamedNodeRef::new_unchecked(DEC_DEFINITION_FORM);
    vec![
        Quad::new(session_iri.clone(), rdf_type, session_class, g.clone()),
        Quad::new(session_iri.clone(), rdf_type, activity, g.clone()),
        Quad::new(
            session_iri.clone(),
            p_source,
            Literal::new_simple_literal(source_label),
            g.clone(),
        ),
        Quad::new(
            session_iri.clone(),
            derived,
            Literal::new_simple_literal(source_label),
            g.clone(),
        ),
        Quad::new(
            session_iri.clone(),
            p_hash,
            Literal::new_simple_literal(definition_hash),
            g.clone(),
        ),
        Quad::new(
            session_iri.clone(),
            p_ver,
            Literal::new_simple_literal(ontology_version),
            g.clone(),
        ),
        Quad::new(
            session_iri.clone(),
            p_form,
            Literal::new_simple_literal(form),
            g.clone(),
        ),
        Quad::new(
            session_iri.clone(),
            at_time,
            Literal::new_simple_literal(started_at),
            g.clone(),
        ),
    ]
}

pub(super) fn sha256_hex(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    let d = h.finalize();
    let mut s = String::with_capacity(d.len() * 2);
    for b in d {
        use std::fmt::Write;
        let _ = write!(s, "{b:02x}");
    }
    s
}

pub(super) fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

/// Seed the v0 bootstrap subscriptions (FT-009 §Behaviour step 4).
pub(super) fn seed_bootstrap_subscriptions(
    store: &Store,
) -> Result<(), oxigraph::store::StorageError> {
    let subs_graph: GraphName =
        NamedNode::new_unchecked(oxi_events::vocab::IRI_OXI_GRAPH_SUBSCRIPTIONS).into();
    let rdf_type = NamedNode::new_unchecked(RDF_TYPE);
    let sub_class = NamedNode::new_unchecked(oxi_events::vocab::IRI_OXI_SUBSCRIPTION);
    let select_pred = NamedNode::new_unchecked(oxi_events::vocab::IRI_OXI_SUB_SELECT_QUERY);
    let mode_pred = NamedNode::new_unchecked(oxi_events::vocab::IRI_OXI_SUB_MODE);
    let rdfs_label = NamedNode::new_unchecked("http://www.w3.org/2000/01/rdf-schema#label");

    let entries: &[(&str, &str, &str)] = &[
        (
            "https://decision-cli.dev/ns/subscription/dispatch-available-code-writer",
            "dispatch available for code-writer",
            "PREFIX dec: <https://decision-cli.dev/ns#> \
             SELECT ?d WHERE { GRAPH ?g { ?d a dec:Dispatch ; dec:role \"code-writer\" } }",
        ),
        (
            "https://decision-cli.dev/ns/subscription/dispatch-completed-code-writer",
            "code-writer dispatch completed",
            "PREFIX dec: <https://decision-cli.dev/ns#> \
             SELECT ?s WHERE { GRAPH ?g { ?s a dec:Session ; dec:role \"code-writer\" ; dec:status \"complete\" } }",
        ),
    ];

    store.transaction(|mut tx| {
        for (iri, label, query) in entries {
            let sub = NamedNode::new_unchecked(*iri);
            tx.insert(
                Quad::new(sub.clone(), rdf_type.clone(), sub_class.clone(), subs_graph.clone())
                    .as_ref(),
            )?;
            tx.insert(
                Quad::new(
                    sub.clone(),
                    select_pred.clone(),
                    Literal::new_simple_literal(*query),
                    subs_graph.clone(),
                )
                .as_ref(),
            )?;
            tx.insert(
                Quad::new(
                    sub.clone(),
                    mode_pred.clone(),
                    Literal::new_simple_literal(oxi_events::vocab::SUB_MODE_INLINE),
                    subs_graph.clone(),
                )
                .as_ref(),
            )?;
            tx.insert(
                Quad::new(
                    sub,
                    rdfs_label.clone(),
                    Literal::new_simple_literal(*label),
                    subs_graph.clone(),
                )
                .as_ref(),
            )?;
        }
        Ok::<_, oxigraph::store::StorageError>(())
    })
}

/// Serialise the orchestration store to disk atomically: write a temp
/// `.dec.tmp-init/` directory next to `workdir`, then rename it to
/// `<workdir>/.dec/`.
pub(super) fn finalise_orchestration_dir(
    workdir: &Path,
    dec_dir: &Path,
    store: &Store,
    definition_bytes: &[u8],
    metadata_json: &str,
) -> Result<(), InitError> {
    let tmp_dec = workdir.join(".dec.tmp-init");
    if tmp_dec.exists() {
        fs::remove_dir_all(&tmp_dec).map_err(|e| InitError::PersistFailed(e.to_string()))?;
    }
    fs::create_dir_all(tmp_dec.join("store"))
        .map_err(|e| InitError::PersistFailed(e.to_string()))?;
    let tmp_dump = tmp_dec.join("store").join("orchestration.nq");
    let mut buf = Vec::new();
    store
        .dump_to_writer(RdfFormat::NQuads, &mut buf)
        .map_err(|e| InitError::PersistFailed(e.to_string()))?;
    fs::write(&tmp_dump, &buf).map_err(|e| InitError::PersistFailed(e.to_string()))?;
    fs::write(tmp_dec.join("definition.ttl"), definition_bytes)
        .map_err(|e| InitError::PersistFailed(e.to_string()))?;
    fs::write(tmp_dec.join("init-metadata.json"), metadata_json)
        .map_err(|e| InitError::PersistFailed(e.to_string()))?;
    fs::rename(&tmp_dec, dec_dir).map_err(|e| InitError::PersistFailed(e.to_string()))?;
    Ok(())
}
