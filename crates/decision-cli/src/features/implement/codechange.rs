//! Persist the worker's [`CodeChange`](super::worker::CodeChangeJson)
//! into the product-cli graph slice with PROV-O lineage (TC-013).

use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use oxigraph::io::RdfFormat;
use oxigraph::model::{GraphName, Literal, NamedNode, NamedNodeRef, Quad};
use oxigraph::store::Store;

use crate::core::vocab::IRI_DEC_GRAPH_ORCHESTRATION;

use super::vocab::{
    DEC_CHANGED_FILE, DEC_CODE_CHANGE_CLASS, DEC_DISPATCH_PROP, DEC_FEATURE_ID, DEC_FILE_PATH_PROP,
    DEC_FILE_SUMMARY_PROP, PROV_GENERATED_BY, RDF_TYPE,
};
use super::worker::{CodeChangeJson, FileWriteJson};

/// Stable IRI of the product-cli CodeChange named graph.
const PRODUCT_CODECHANGE_GRAPH_IRI: &str = "https://product-meta/graph/code-changes";

/// IRI for the `dec:summary` predicate on `CodeChange`.
const DEC_SUMMARY_PROP: &str = "https://decision-cli.dev/ns#summary";

/// IRI for the `dec:bytesWritten` predicate on file nodes.
const DEC_BYTES_WRITTEN_PROP: &str = "https://decision-cli.dev/ns#bytesWritten";

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
    let g = NamedNode::new(PRODUCT_CODECHANGE_GRAPH_IRI).context("product graph IRI")?;
    let store = open_codechange_store(path)?;
    let quads = build_codechange_quads(
        code_change,
        session_iri,
        dispatch_iri,
        feature_id,
        GraphName::NamedNode(g),
    )?;
    insert_codechange_quads(&store, &quads, path)?;
    dump_codechange_store(&store, path)?;
    let _ = IRI_DEC_GRAPH_ORCHESTRATION;
    Ok(())
}

/// Open a product-CodeChange [`Store`], pre-populating it from `path`
/// when the file already exists with non-empty contents.
fn open_codechange_store(path: &Path) -> Result<Store> {
    let store = Store::new().context("opening product codechange store")?;
    if path.exists() {
        let bytes = fs::read(path).with_context(|| format!("reading {}", path.display()))?;
        if !bytes.is_empty() {
            store
                .load_from_reader(RdfFormat::NQuads, bytes.as_slice())
                .with_context(|| format!("loading {}", path.display()))?;
        }
    }
    Ok(store)
}

/// Insert the freshly built CodeChange triples in a single transaction.
fn insert_codechange_quads(store: &Store, quads: &[Quad], path: &Path) -> Result<()> {
    store
        .transaction(|mut tx| {
            for q in quads {
                tx.insert(q.as_ref())?;
            }
            Ok::<_, oxigraph::store::StorageError>(())
        })
        .with_context(|| format!("inserting CodeChange triples into {}", path.display()))?;
    Ok(())
}

/// Dump the product-CodeChange [`Store`] to `path` atomically via
/// write-then-rename through a sibling `.nq.tmp`.
fn dump_codechange_store(store: &Store, path: &Path) -> Result<()> {
    let mut buf: Vec<u8> = Vec::new();
    store
        .dump_to_writer(RdfFormat::NQuads, &mut buf)
        .context("dumping product CodeChange store")?;
    let tmp = path.with_extension("nq.tmp");
    fs::write(&tmp, &buf).with_context(|| format!("writing {}", tmp.display()))?;
    fs::rename(&tmp, path)
        .with_context(|| format!("renaming {} -> {}", tmp.display(), path.display()))?;
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
    let mut quads = codechange_header_quads(
        &cc_iri,
        session_iri,
        dispatch_iri,
        feature_id,
        code_change,
        &graph,
    );
    for file in &code_change.files {
        let file_quads = codechange_file_quads(&cc_iri, file, &code_change.iri, &graph)?;
        quads.extend(file_quads);
    }
    Ok(quads)
}

/// The five core triples describing the `CodeChange` itself — type,
/// PROV-O lineage to the [`Session`], the originating [`Dispatch`],
/// feature_id tag, worker summary.
fn codechange_header_quads(
    cc_iri: &NamedNode,
    session_iri: &NamedNode,
    dispatch_iri: &NamedNode,
    feature_id: &str,
    code_change: &CodeChangeJson,
    graph: &GraphName,
) -> Vec<Quad> {
    let class = pred(DEC_CODE_CHANGE_CLASS);
    let feature_lit = Literal::new_simple_literal(feature_id);
    let summary_lit = Literal::new_simple_literal(&code_change.summary);
    vec![
        node_quad(cc_iri, &pred(RDF_TYPE), &class, graph),
        node_quad(cc_iri, &pred(PROV_GENERATED_BY), session_iri, graph),
        node_quad(cc_iri, &pred(DEC_DISPATCH_PROP), dispatch_iri, graph),
        literal_quad(cc_iri, &pred(DEC_FEATURE_ID), feature_lit, graph),
        literal_quad(cc_iri, &pred(DEC_SUMMARY_PROP), summary_lit, graph),
    ]
}

/// Owned [`NamedNode`] from a static IRI string.
fn pred(iri: &str) -> NamedNode {
    NamedNodeRef::new_unchecked(iri).into_owned()
}

/// Quad whose object is a [`NamedNode`].
fn node_quad(s: &NamedNode, p: &NamedNode, o: &NamedNode, g: &GraphName) -> Quad {
    Quad::new(s.clone(), p.clone(), o.clone(), g.clone())
}

/// Quad whose object is a [`Literal`].
fn literal_quad(s: &NamedNode, p: &NamedNode, o: Literal, g: &GraphName) -> Quad {
    Quad::new(s.clone(), p.clone(), o, g.clone())
}

/// Per-file triples — the `dec:changedFile` edge plus path, optional
/// summary, optional bytesWritten on the freshly minted file node.
fn codechange_file_quads(
    cc_iri: &NamedNode,
    file: &FileWriteJson,
    code_change_iri: &str,
    graph: &GraphName,
) -> Result<Vec<Quad>> {
    let file_node = mint_file_node(code_change_iri, &file.path)?;
    let mut quads = vec![
        node_quad(cc_iri, &pred(DEC_CHANGED_FILE), &file_node, graph),
        literal_quad(
            &file_node,
            &pred(DEC_FILE_PATH_PROP),
            Literal::new_simple_literal(&file.path),
            graph,
        ),
    ];
    if !file.summary.is_empty() {
        quads.push(literal_quad(
            &file_node,
            &pred(DEC_FILE_SUMMARY_PROP),
            Literal::new_simple_literal(&file.summary),
            graph,
        ));
    }
    if file.bytes_written > 0 {
        quads.push(literal_quad(
            &file_node,
            &pred(DEC_BYTES_WRITTEN_PROP),
            Literal::new_simple_literal(file.bytes_written.to_string()),
            graph,
        ));
    }
    Ok(quads)
}

/// Mint a stable file IRI under the parent code-change IRI.
fn mint_file_node(code_change_iri: &str, path: &str) -> Result<NamedNode> {
    NamedNode::new(format!(
        "{}/file/{}",
        code_change_iri,
        sanitize_iri_tail(path)
    ))
    .with_context(|| format!("minting file IRI for {path}"))
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
