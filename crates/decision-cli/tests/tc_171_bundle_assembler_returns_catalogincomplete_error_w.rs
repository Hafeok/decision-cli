//! TC-171 — Bundle assembler returns `CatalogIncomplete` error when a
//! mandatory field has zero artifacts and no default.
//!
//! Validates: FT-102 · ADR-066.
//! Spec: `.product/tests/TC-171-bundle-assembler-returns-catalogincomplete-error-w.md`
//!
//! Strict catalog-required mode is opt-in via the
//! `DEC_VERIFY_REQUIRE_CATALOG=1` env var. The test exercises both
//! enforcement (strict mode + empty catalog ⇒ error) and the lenient
//! default (no env var ⇒ warning only, advisory for `exemplar_graphs`).
//! `exemplar_graphs` is advisory in either mode per the ADR.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use decision_cli::core::handler::Error as HandlerError;
use decision_cli::core::ontology::catalog::{CapabilityReference, OntologyDescription};
use decision_cli::core::store::{load_store_from_dump, orchestration_dump_path, persist_store};
use decision_cli::init::{run as init_run, DefinitionSource};
use decision_cli::verify_graph_generate::bundle::assemble_enrichment_for;
use decision_cli::verify_graph_generate::enrichment::set_strict_override;
use decision_cli::StreamWriter;
use oxi_events::Mutation;
use oxigraph::model::NamedNode;

const STREAM_TTL: &str =
    include_str!("../src/core/bundled/assets/streams/engineering-development.ttl");

static COUNTER: AtomicU64 = AtomicU64::new(0);

struct WorkdirGuard(PathBuf);

impl WorkdirGuard {
    fn new(tag: &str) -> Self {
        let mut base = env::temp_dir();
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        let counter = COUNTER.fetch_add(1, Ordering::Relaxed);
        base.push(format!(
            "decision-cli-tc171-{tag}-{}-{}-{}",
            std::process::id(),
            nanos,
            counter,
        ));
        fs::create_dir_all(&base).expect("create workdir");
        Self(base)
    }
    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for WorkdirGuard {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn write_seed_definition(dir: &Path) -> PathBuf {
    let p = dir.join("stream.ttl");
    fs::write(&p, STREAM_TTL).expect("write seed");
    p
}

fn seed_capability_only(workdir: &Path) {
    let dump = orchestration_dump_path(workdir);
    let store = std::sync::Arc::new(load_store_from_dump(&dump).expect("load store"));
    let stream = NamedNode::new("https://decision-cli.dev/stream/decision-cli-development")
        .expect("stream iri");
    let writer =
        StreamWriter::bootstrap(std::sync::Arc::clone(&store), stream).expect("stream writer");
    let cr = CapabilityReference {
        id: "CR-001".to_string(),
        command: "dec verify graph new".to_string(),
        capability_version: "0.3.0".to_string(),
        body: r#"{"command":"dec verify graph new","flags":[],"exit_codes":[]}"#.to_string(),
        supersedes: None,
    };
    writer
        .commit(Mutation::insert(cr.to_quads()))
        .expect("commit CR");
    persist_store(&store, &dump).expect("persist");
}

fn seed_ontology_only(workdir: &Path) {
    let dump = orchestration_dump_path(workdir);
    let store = std::sync::Arc::new(load_store_from_dump(&dump).expect("load store"));
    let stream = NamedNode::new("https://decision-cli.dev/stream/decision-cli-development")
        .expect("stream iri");
    let writer =
        StreamWriter::bootstrap(std::sync::Arc::clone(&store), stream).expect("stream writer");
    let od = OntologyDescription {
        id: "OD-001".to_string(),
        namespace: "https://decision-cli.dev/ns#".to_string(),
        prefix: "dec".to_string(),
        ontology_version: "0.3.0".to_string(),
        body: r#"{"classes":[]}"#.to_string(),
        supersedes: None,
    };
    writer
        .commit(Mutation::insert(od.to_quads()))
        .expect("commit OD");
    persist_store(&store, &dump).expect("persist");
}

#[test]
fn tc_171_bundle_assembler_returns_catalogincomplete_error_w() {
    // Headline composes the strict-mode scenarios under one entrypoint
    // so the runner can target a single test name.
    scenario_a_empty_cli_surface_fails_before_worker_dispatch();
    scenario_b_empty_ontology_vocabulary_fails();
    scenario_c_multiple_missing_fields_batched();
    scenario_d_empty_exemplars_is_not_an_error();
    scenario_e_lenient_default_keeps_legacy_paths_working();
}

#[test]
fn scenario_a_empty_cli_surface_fails_before_worker_dispatch() {
    let _strict = set_strict_override(true);
    let wd = WorkdirGuard::new("scen-a");
    let seed = write_seed_definition(wd.path());
    init_run(wd.path(), DefinitionSource::File(seed)).expect("dec init");

    let err = assemble_enrichment_for(wd.path(), None, "missing-env")
        .expect_err("must return CatalogIncomplete with strict mode + empty catalog");
    match err {
        HandlerError::Internal { detail } => {
            assert!(
                detail.contains("CatalogIncomplete"),
                "error must be marked CatalogIncomplete; got {detail}",
            );
            assert!(
                detail.contains("cli_surface"),
                "missing_fields must list cli_surface; got {detail}",
            );
        }
        other => panic!("expected Internal/CatalogIncomplete, got {other:?}"),
    }
}

#[test]
fn scenario_b_empty_ontology_vocabulary_fails() {
    let _strict = set_strict_override(true);
    let wd = WorkdirGuard::new("scen-b");
    let seed = write_seed_definition(wd.path());
    init_run(wd.path(), DefinitionSource::File(seed)).expect("dec init");

    // Seed CR but no OD — assembler should still fail on ontology_vocabulary.
    seed_capability_only(wd.path());

    let err = assemble_enrichment_for(wd.path(), None, "missing-env")
        .expect_err("must return CatalogIncomplete when OD is missing");
    match err {
        HandlerError::Internal { detail } => {
            assert!(
                detail.contains("ontology_vocabulary"),
                "missing_fields must list ontology_vocabulary; got {detail}",
            );
        }
        other => panic!("expected Internal/CatalogIncomplete, got {other:?}"),
    }
}

#[test]
fn scenario_c_multiple_missing_fields_batched() {
    let _strict = set_strict_override(true);
    let wd = WorkdirGuard::new("scen-c");
    let seed = write_seed_definition(wd.path());
    init_run(wd.path(), DefinitionSource::File(seed)).expect("dec init");

    let err = assemble_enrichment_for(wd.path(), None, "missing-env")
        .expect_err("two missing fields ⇒ one batched error");
    match err {
        HandlerError::Internal { detail } => {
            assert!(detail.contains("cli_surface"));
            assert!(detail.contains("ontology_vocabulary"));
        }
        other => panic!("expected Internal/CatalogIncomplete, got {other:?}"),
    }
}

#[test]
fn scenario_d_empty_exemplars_is_not_an_error() {
    let _strict = set_strict_override(true);
    let wd = WorkdirGuard::new("scen-d");
    let seed = write_seed_definition(wd.path());
    init_run(wd.path(), DefinitionSource::File(seed)).expect("dec init");

    // Seed CR + OD but no exemplars — assembler should succeed.
    seed_capability_only(wd.path());
    seed_ontology_only(wd.path());

    let enrichment = assemble_enrichment_for(wd.path(), None, "any-env")
        .expect("CR + OD present, no exemplars ⇒ success");
    assert!(enrichment.exemplar_graphs.is_empty());
    // No env supplied ⇒ the "no env" warning fires; that's fine.
    assert!(!enrichment.bundle_metadata.warnings.is_empty());
}

#[test]
fn scenario_e_lenient_default_keeps_legacy_paths_working() {
    // Explicitly disable strict mode for this thread.
    let _strict = set_strict_override(false);
    let wd = WorkdirGuard::new("scen-e");
    let seed = write_seed_definition(wd.path());
    init_run(wd.path(), DefinitionSource::File(seed)).expect("dec init");

    let enrichment = assemble_enrichment_for(wd.path(), None, "missing-env")
        .expect("lenient default must not error on empty catalog");

    assert!(enrichment.cli_surface.commands.is_empty());
    assert!(enrichment.ontology_vocabulary.namespace.is_empty());
    let joined = enrichment.bundle_metadata.warnings.join(" | ");
    assert!(
        joined.contains("catalog is empty for cli_surface"),
        "lenient mode should record a warning; got {joined:?}",
    );
}
