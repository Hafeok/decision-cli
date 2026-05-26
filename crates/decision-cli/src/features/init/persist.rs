//! Persistence helpers: copy triples, session-quads builder, subscription
//! seeding, hashing.

use std::fs;
use std::path::Path;

use oxigraph::io::RdfFormat;
use oxigraph::model::{GraphName, GraphNameRef, Literal, NamedNode, NamedNodeRef, Quad};
use oxigraph::store::Store;
use sha2::{Digest, Sha256};

use super::vocab::{
    DEC_DEFINITION_FORM, DEC_DEFINITION_HASH, DEC_DEFINITION_SOURCE, DEC_MANIFEST_SHA256,
    DEC_ONTOLOGY_VERSION, DEC_SESSION_CLASS, PROV_ACTIVITY, PROV_AT_TIME, PROV_DERIVED_FROM,
    RDF_TYPE,
};
use super::InitError;
use crate::core::worker::manifest_sha256_hex;

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
    let mut quads = build_session_type_quads(session_iri, graph);
    quads.extend(build_session_source_quads(session_iri, graph, source_label));
    quads.extend(build_session_metadata_quads(
        session_iri,
        graph,
        definition_hash,
        ontology_version,
        form,
        started_at,
    ));
    quads
}

fn build_session_type_quads(session_iri: &NamedNode, graph: &GraphName) -> Vec<Quad> {
    let rdf_type = NamedNodeRef::new_unchecked(RDF_TYPE);
    let session_class = NamedNodeRef::new_unchecked(DEC_SESSION_CLASS);
    let activity = NamedNodeRef::new_unchecked(PROV_ACTIVITY);
    vec![
        Quad::new(session_iri.clone(), rdf_type, session_class, graph.clone()),
        Quad::new(session_iri.clone(), rdf_type, activity, graph.clone()),
    ]
}

fn build_session_source_quads(
    session_iri: &NamedNode,
    graph: &GraphName,
    source_label: &str,
) -> Vec<Quad> {
    let derived = NamedNodeRef::new_unchecked(PROV_DERIVED_FROM);
    let p_source = NamedNodeRef::new_unchecked(DEC_DEFINITION_SOURCE);
    vec![
        Quad::new(
            session_iri.clone(),
            p_source,
            Literal::new_simple_literal(source_label),
            graph.clone(),
        ),
        Quad::new(
            session_iri.clone(),
            derived,
            Literal::new_simple_literal(source_label),
            graph.clone(),
        ),
    ]
}

fn build_session_metadata_quads(
    session_iri: &NamedNode,
    graph: &GraphName,
    definition_hash: &str,
    ontology_version: &str,
    form: &str,
    started_at: &str,
) -> Vec<Quad> {
    let manifest_hash = manifest_sha256_hex();
    vec![
        session_literal_quad(session_iri, graph, DEC_DEFINITION_HASH, definition_hash),
        session_literal_quad(session_iri, graph, DEC_ONTOLOGY_VERSION, ontology_version),
        session_literal_quad(session_iri, graph, DEC_DEFINITION_FORM, form),
        session_literal_quad(session_iri, graph, PROV_AT_TIME, started_at),
        session_literal_quad(session_iri, graph, DEC_MANIFEST_SHA256, &manifest_hash),
    ]
}

fn session_literal_quad(
    session_iri: &NamedNode,
    graph: &GraphName,
    predicate_iri: &str,
    value: &str,
) -> Quad {
    Quad::new(
        session_iri.clone(),
        NamedNodeRef::new_unchecked(predicate_iri),
        Literal::new_simple_literal(value),
        graph.clone(),
    )
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

/// Seed the v0 bootstrap subscriptions (FT-009 §Behaviour step 4) plus
/// the FT-022 verifier-dispatch subscription (lives alongside the v0
/// pair so the slice-2 substrate is available from the first dispatch).
pub(super) fn seed_bootstrap_subscriptions(
    store: &Store,
) -> Result<(), oxigraph::store::StorageError> {
    let subs_graph: GraphName =
        NamedNode::new_unchecked(oxi_events::vocab::IRI_OXI_GRAPH_SUBSCRIPTIONS).into();
    let entries = bootstrap_subscription_entries();
    let mut quads: Vec<Quad> = Vec::with_capacity(entries.len() * 4);
    for (iri, label, query) in entries {
        quads.extend(build_subscription_quads(iri, label, query, &subs_graph));
    }
    // FT-022: verifier-dispatch subscription owns its own quad builder
    // because it carries `oxi:handler` + `oxi:mode "async"`, which the
    // slice-1 v0 subscriptions do not. Reusing
    // `build_subscription_quads` would lose those fields.
    quads.extend(crate::core::subscriptions::verifier_dispatch::seed_quads());
    // FT-029: feedback-routing subscription seed lives alongside the
    // verifier-dispatch seed so the routing handler is wired from the
    // first `dec init`. Same pattern as FT-022 — own quad builder
    // because it carries `oxi:handler`.
    quads.extend(crate::core::feedback::routing::seed_quads());
    // FT-032 / ADR-025: feedback-resume subscription seed. Watches for
    // dec:Feedback artifacts whose lifecycle reaches a terminal state
    // (addressed / rejected / closed) when referenced by a paused
    // DispatchGroup via dec:blockedBy. The handler advances the group
    // back to awaiting-action (retry) or feedback-rejected-action-blocked
    // (terminal failure).
    quads.extend(crate::core::subscriptions::feedback_resume::seed_quads());
    // FT-050 / ADR-030: verify-graph-author auto-dispatch subscription
    // seed. Fires on dec:Feature create/update events; the handler
    // enumerates configured envs, consults the coverage primitive and
    // dedup ledger, and emits one VerifyGraphAuthorDispatchEvent per
    // uncovered (feature, env) pair.
    quads.extend(crate::core::subscriptions::verify_graph_author_dispatch::seed_quads());
    // FT-100: verify-graph-runner auto-dispatch subscriptions.
    // graph_accepted_dispatch fires on dec:VerificationGraph create/update;
    // code_change_committed_dispatch fires on dec:CodeChangeCommitted.
    quads.extend(crate::core::subscriptions::graph_accepted_dispatch::seed_quads());
    quads.extend(crate::core::subscriptions::code_change_committed_dispatch::seed_quads());
    store.transaction(|mut tx| {
        for q in &quads {
            tx.insert(q.as_ref())?;
        }
        Ok::<_, oxigraph::store::StorageError>(())
    })
}

fn bootstrap_subscription_entries() -> &'static [(&'static str, &'static str, &'static str)] {
    &[
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
    ]
}

fn build_subscription_quads(
    iri: &str,
    label: &str,
    query: &str,
    subs_graph: &GraphName,
) -> Vec<Quad> {
    let rdf_type = NamedNode::new_unchecked(RDF_TYPE);
    let sub_class = NamedNode::new_unchecked(oxi_events::vocab::IRI_OXI_SUBSCRIPTION);
    let select_pred = NamedNode::new_unchecked(oxi_events::vocab::IRI_OXI_SUB_SELECT_QUERY);
    let mode_pred = NamedNode::new_unchecked(oxi_events::vocab::IRI_OXI_SUB_MODE);
    let rdfs_label = NamedNode::new_unchecked("http://www.w3.org/2000/01/rdf-schema#label");
    let sub = NamedNode::new_unchecked(iri);
    vec![
        Quad::new(sub.clone(), rdf_type, sub_class, subs_graph.clone()),
        Quad::new(
            sub.clone(),
            select_pred,
            Literal::new_simple_literal(query),
            subs_graph.clone(),
        ),
        Quad::new(
            sub.clone(),
            mode_pred,
            Literal::new_simple_literal(oxi_events::vocab::SUB_MODE_INLINE),
            subs_graph.clone(),
        ),
        Quad::new(
            sub,
            rdfs_label,
            Literal::new_simple_literal(label),
            subs_graph.clone(),
        ),
    ]
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
    seed_verify_env_files(&tmp_dec)?;
    fs::rename(&tmp_dec, dec_dir).map_err(|e| InitError::PersistFailed(e.to_string()))?;
    Ok(())
}

/// FT-035 / ADR-028 — write the `.dec/verify/env/ENV-001-ephemeral-cli.ttl`
/// seed file in canonical Turtle form. Byte-stable across runs so
/// re-initialising in a fresh tempdir produces the same bytes.
fn seed_verify_env_files(tmp_dec: &Path) -> Result<(), InitError> {
    use crate::core::ontology::verification_env::{
        ephemeral_cli_env, to_canonical_turtle, EPHEMERAL_CLI_ENV_FILENAME,
    };
    let env_dir = tmp_dec.join("verify").join("env");
    fs::create_dir_all(&env_dir).map_err(|e| InitError::PersistFailed(e.to_string()))?;
    let ttl = to_canonical_turtle(&ephemeral_cli_env());
    let target = env_dir.join(EPHEMERAL_CLI_ENV_FILENAME);
    fs::write(&target, ttl.as_bytes()).map_err(|e| InitError::PersistFailed(e.to_string()))?;
    Ok(())
}
