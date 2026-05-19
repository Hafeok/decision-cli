//! Persist the worker's [`CodeChange`](super::worker::CodeChangeJson)
//! into the product-cli graph slice with PROV-O lineage (TC-013).

use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use oxigraph::io::RdfFormat;
use oxigraph::model::{GraphName, Literal, NamedNode, NamedNodeRef, Quad};
use oxigraph::store::Store;

use crate::vocab::IRI_DEC_GRAPH_ORCHESTRATION;

use super::vocab::{
    DEC_CHANGED_FILE, DEC_CODE_CHANGE_CLASS, DEC_DISPATCH_PROP, DEC_FEATURE_ID,
    DEC_FILE_PATH_PROP, DEC_FILE_SUMMARY_PROP, PROV_GENERATED_BY, RDF_TYPE,
};
use super::worker::CodeChangeJson;

pub(super) fn write_codechange_to_product_graph(
    path: &Path,
    code_change: &CodeChangeJson,
    session_iri: &NamedNode,
    dispatch_iri: &NamedNode,
    feature_id: &str,
) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;
    }
    let product_graph_iri = "https://product-meta/graph/code-changes";
    let g = NamedNode::new(product_graph_iri).context("product graph IRI")?;
    let store = Store::new().context("opening product codechange store")?;
    if path.exists() {
        let bytes = fs::read(path).with_context(|| format!("reading {}", path.display()))?;
        if !bytes.is_empty() {
            store
                .load_from_reader(RdfFormat::NQuads, bytes.as_slice())
                .with_context(|| format!("loading {}", path.display()))?;
        }
    }
    let quads = build_codechange_quads(
        code_change,
        session_iri,
        dispatch_iri,
        feature_id,
        GraphName::NamedNode(g.clone()),
    )?;
    store
        .transaction(|mut tx| {
            for q in &quads {
                tx.insert(q.as_ref())?;
            }
            Ok::<_, oxigraph::store::StorageError>(())
        })
        .with_context(|| format!("inserting CodeChange triples into {}", path.display()))?;
    let mut buf: Vec<u8> = Vec::new();
    store
        .dump_to_writer(RdfFormat::NQuads, &mut buf)
        .context("dumping product CodeChange store")?;
    let tmp = path.with_extension("nq.tmp");
    fs::write(&tmp, &buf).with_context(|| format!("writing {}", tmp.display()))?;
    fs::rename(&tmp, path)
        .with_context(|| format!("renaming {} -> {}", tmp.display(), path.display()))?;
    let _ = IRI_DEC_GRAPH_ORCHESTRATION;
    Ok(())
}

fn build_codechange_quads(
    code_change: &CodeChangeJson,
    session_iri: &NamedNode,
    dispatch_iri: &NamedNode,
    feature_id: &str,
    graph: GraphName,
) -> Result<Vec<Quad>> {
    let cc_iri = NamedNode::new(&code_change.iri).context("code change IRI")?;
    let class = NamedNodeRef::new_unchecked(DEC_CODE_CHANGE_CLASS).into_owned();
    let rdf_type = NamedNodeRef::new_unchecked(RDF_TYPE).into_owned();
    let prov_generated = NamedNodeRef::new_unchecked(PROV_GENERATED_BY).into_owned();
    let dispatch_pred = NamedNodeRef::new_unchecked(DEC_DISPATCH_PROP).into_owned();
    let feature_pred = NamedNodeRef::new_unchecked(DEC_FEATURE_ID).into_owned();
    let file_pred = NamedNodeRef::new_unchecked(DEC_CHANGED_FILE).into_owned();
    let path_pred = NamedNodeRef::new_unchecked(DEC_FILE_PATH_PROP).into_owned();
    let summary_pred = NamedNodeRef::new_unchecked(DEC_FILE_SUMMARY_PROP).into_owned();
    let mut quads: Vec<Quad> = vec![
        Quad::new(cc_iri.clone(), rdf_type, class, graph.clone()),
        Quad::new(
            cc_iri.clone(),
            prov_generated,
            session_iri.clone(),
            graph.clone(),
        ),
        Quad::new(
            cc_iri.clone(),
            dispatch_pred,
            dispatch_iri.clone(),
            graph.clone(),
        ),
        Quad::new(
            cc_iri.clone(),
            feature_pred,
            Literal::new_simple_literal(feature_id),
            graph.clone(),
        ),
        Quad::new(
            cc_iri.clone(),
            NamedNodeRef::new_unchecked("https://decision-cli.dev/ns#summary").into_owned(),
            Literal::new_simple_literal(&code_change.summary),
            graph.clone(),
        ),
    ];
    for file in &code_change.files {
        let file_node = NamedNode::new(format!(
            "{}/file/{}",
            code_change.iri,
            sanitize_iri_tail(&file.path)
        ))
        .with_context(|| format!("minting file IRI for {}", file.path))?;
        quads.push(Quad::new(
            cc_iri.clone(),
            file_pred.clone(),
            file_node.clone(),
            graph.clone(),
        ));
        quads.push(Quad::new(
            file_node.clone(),
            path_pred.clone(),
            Literal::new_simple_literal(&file.path),
            graph.clone(),
        ));
        if !file.summary.is_empty() {
            quads.push(Quad::new(
                file_node.clone(),
                summary_pred.clone(),
                Literal::new_simple_literal(&file.summary),
                graph.clone(),
            ));
        }
        if file.bytes_written > 0 {
            quads.push(Quad::new(
                file_node,
                NamedNodeRef::new_unchecked("https://decision-cli.dev/ns#bytesWritten")
                    .into_owned(),
                Literal::new_simple_literal(file.bytes_written.to_string()),
                graph.clone(),
            ));
        }
    }
    Ok(quads)
}

fn sanitize_iri_tail(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '/' | '\\' | ' ' | '#' | '?' | '%' | '&' => out.push('_'),
            c if c.is_ascii_alphanumeric() || c == '-' || c == '.' || c == '_' => out.push(c),
            _ => out.push('_'),
        }
    }
    if out.is_empty() {
        out.push('_');
    }
    out
}
