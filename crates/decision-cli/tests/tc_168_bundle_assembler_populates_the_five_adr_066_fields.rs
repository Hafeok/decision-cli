//! TC-168 — Bundle assembler populates the five ADR-066 fields from
//! catalog artifacts via SPARQL CONSTRUCT.
//!
//! Validates: FT-102 · ADR-066.
//! Spec: `.product/tests/TC-168-bundle-assembler-populates-the-five-adr-066-fields.md`
//!
//! Scenario A (full enrichment) is the primary assertion path: the test
//! seeds a populated catalog + an env with a `dec:concreteCapabilities`
//! block, runs the bundle assembler, and verifies all five fields plus
//! the metadata block carry the expected shape. Scenarios B and D are
//! exercised as separate `#[test]`s.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use decision_cli::core::ontology::catalog::{
    CapabilityReference, ExemplarGraph, OntologyDescription, SafetyClassTag,
};
use decision_cli::core::store::{
    load_store_from_dump, orchestration_dump_path, persist_store,
};
use decision_cli::init::{run as init_run, DefinitionSource};
use decision_cli::verify_graph_generate::bundle::assemble_enrichment_for;
use decision_cli::vocab::{
    IRI_DEC_RESULT_OF, IRI_DEC_VERDICT, IRI_DEC_VERIFICATION_GRAPH_RESULT, VERDICT_APPROVED,
};
use decision_cli::StreamWriter;
use oxi_events::Mutation;
use oxigraph::model::{GraphName, Literal, NamedNode, NamedNodeRef, Quad};

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
            "decision-cli-tc168-{tag}-{}-{}-{}",
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

/// Seed a populated catalog through the StreamWriter so the SHACL
/// chokepoint runs end-to-end. Returns nothing — the store dump is
/// persisted to disk at the workdir's `.dec/store/orchestration.nq`.
fn seed_catalog(workdir: &Path) {
    let dump = orchestration_dump_path(workdir);
    let store = Arc::new(load_store_from_dump(&dump).expect("load store"));
    let stream = NamedNode::new("https://decision-cli.dev/stream/decision-cli-development")
        .expect("stream iri");
    let writer =
        StreamWriter::bootstrap(Arc::clone(&store), stream).expect("stream writer");

    // CR-001..CR-003 — three commands at dec 0.3.0.
    for (id, cmd) in [
        ("CR-001", "dec verify graph new"),
        ("CR-002", "dec verify graph run"),
        ("CR-003", "dec sparql query"),
    ] {
        let cr = CapabilityReference {
            id: id.to_string(),
            command: cmd.to_string(),
            capability_version: "0.3.0".to_string(),
            body: format!(
                r#"{{"command":"{cmd}","synopsis":"FT-102 fixture","flags":[],"exit_codes":[{{"code":0,"meaning":"ok"}}],"observable_effects":[]}}"#
            ),
            supersedes: None,
        };
        writer
            .commit(Mutation::insert(cr.to_quads()))
            .expect("commit CR");
    }

    // OD-001 — declares dec namespace + three classes.
    let od = OntologyDescription {
        id: "OD-001".to_string(),
        namespace: "https://decision-cli.dev/ns#".to_string(),
        prefix: "dec".to_string(),
        ontology_version: "0.3.0".to_string(),
        body: r#"{
            "namespace":"https://decision-cli.dev/ns#",
            "prefix":"dec",
            "classes":[
                {"local_name":"VerificationGraph"},
                {"local_name":"VerificationStep"},
                {"local_name":"Session"}
            ],
            "ranges_summary":"see Decision-Driven_Design__Entity_Reference.md"
        }"#
        .to_string(),
        supersedes: None,
    };
    writer
        .commit(Mutation::insert(od.to_quads()))
        .expect("commit OD");

    // Seed two approved VGRs so EX-001 / EX-002 can be promoted via
    // the catalog SHACL validator.
    let vg1 = NamedNode::new("https://decision-cli.dev/ns/graph/verify-graph/VG-001").unwrap();
    let vg2 = NamedNode::new("https://decision-cli.dev/ns/graph/verify-graph/VG-002").unwrap();
    let vgr1 = NamedNode::new("https://decision-cli.dev/ns/result/VGR-001").unwrap();
    let vgr2 = NamedNode::new("https://decision-cli.dev/ns/result/VGR-002").unwrap();
    seed_approved_vgr(&store, &vg1, &vgr1);
    seed_approved_vgr(&store, &vg2, &vgr2);

    for (id, vg_iri, vgr_iri, name) in [
        ("EX-001", &vg1, &vgr1, "shell-only-smoke"),
        ("EX-002", &vg2, &vgr2, "sparql-then-shell"),
    ] {
        let ex = ExemplarGraph {
            id: id.to_string(),
            exemplar_of: vg_iri.clone(),
            applies_to_safety_class: SafetyClassTag::Isolated,
            pattern_name: name.to_string(),
            rationale: format!(
                "Canonical exemplar named {name}; pattern proven by a passing run \
                 of the underlying verification graph in an isolated env."
            ),
            based_on_approved_result: vgr_iri.clone(),
            supersedes: None,
        };
        writer
            .commit(Mutation::insert(ex.to_quads()))
            .expect("commit EX");
    }

    persist_store(&store, &dump).expect("persist store");
}

fn seed_approved_vgr(store: &oxigraph::store::Store, vg: &NamedNode, vgr: &NamedNode) {
    const RDF_TYPE: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#type";
    const VGR_GRAPH: &str = "https://decision-cli.dev/ns/graph/verify-result";
    let g = GraphName::NamedNode(NamedNode::new(VGR_GRAPH).unwrap());
    let rdf_type = NamedNodeRef::new_unchecked(RDF_TYPE);
    let cls = NamedNodeRef::new_unchecked(IRI_DEC_VERIFICATION_GRAPH_RESULT);
    let verdict_pred = NamedNodeRef::new_unchecked(IRI_DEC_VERDICT);
    let result_of_pred = NamedNodeRef::new_unchecked(IRI_DEC_RESULT_OF);
    let quads = vec![
        Quad::new(vgr.clone(), rdf_type, cls, g.clone()),
        Quad::new(
            vgr.clone(),
            verdict_pred,
            Literal::new_simple_literal(VERDICT_APPROVED),
            g.clone(),
        ),
        Quad::new(vgr.clone(), result_of_pred, vg.clone(), g),
    ];
    store
        .transaction(|mut tx| {
            for q in &quads {
                tx.insert(q.as_ref())?;
            }
            Ok::<(), oxigraph::store::StorageError>(())
        })
        .expect("seed VGR triples");
}

/// Write an env Turtle file with a `dec:concreteCapabilities` block.
fn write_env_with_concrete(workdir: &Path, env_id: &str) -> PathBuf {
    let dir = workdir.join(".dec").join("verify").join("env");
    fs::create_dir_all(&dir).expect("create env dir");
    let path = dir.join(format!("{env_id}.ttl"));
    let body = format!(
        r#"@prefix dec: <https://decision-cli.dev/ns#> .

<https://decision-cli.dev/ns/env/{env_id}>
    a dec:VerificationEnvironment ;
    dec:envType "ephemeral-tempdir" ;
    dec:safetyClass "isolated" ;
    dec:allowedOps ( "shell" "filesystem" "sparql-local" ) ;
    dec:setup "mkdir -p \"$DEC_VERIFY_TMP\"" ;
    dec:teardown "rm -rf \"$DEC_VERIFY_TMP\"" ;
    dec:concreteCapabilities [
        dec:binariesOnPath        ( "dec" "bash" ) ;
        dec:writablePaths         ( "$DEC_VERIFY_TMP" "./" ) ;
        dec:allowedHosts          ( ) ;
        dec:environmentVariables  ( "DEC_VERIFY_TMP" "PATH" ) ;
        dec:preSeededArtifacts    ( )
    ] .
"#,
    );
    fs::write(&path, body).expect("write env ttl");
    path
}

/// Variant that omits the `dec:concreteCapabilities` block.
fn write_env_without_concrete(workdir: &Path, env_id: &str) -> PathBuf {
    let dir = workdir.join(".dec").join("verify").join("env");
    fs::create_dir_all(&dir).expect("create env dir");
    let path = dir.join(format!("{env_id}.ttl"));
    let body = format!(
        r#"@prefix dec: <https://decision-cli.dev/ns#> .

<https://decision-cli.dev/ns/env/{env_id}>
    a dec:VerificationEnvironment ;
    dec:envType "ephemeral-tempdir" ;
    dec:safetyClass "isolated" ;
    dec:allowedOps ( "shell" "filesystem" "sparql-local" ) ;
    dec:setup "true" ;
    dec:teardown "true" .
"#,
    );
    fs::write(&path, body).expect("write env ttl");
    path
}

#[test]
fn tc_168_bundle_assembler_populates_the_five_adr_066_fields() {
    let wd = WorkdirGuard::new("scenario-a");
    let seed = write_seed_definition(wd.path());
    init_run(wd.path(), DefinitionSource::File(seed)).expect("dec init");

    seed_catalog(wd.path());
    let _env_path = write_env_with_concrete(wd.path(), "ENV-001");
    let env = decision_cli::core::ontology::verification_env::from_turtle(
        &wd.path()
            .join(".dec/verify/env/ENV-001.ttl"),
    )
    .expect("load env");

    let enrichment = assemble_enrichment_for(wd.path(), Some(&env), "ENV-001")
        .expect("assemble enrichment");

    // cli_surface — three commands, version 0.3.0, dec_subcommands list.
    assert_eq!(
        enrichment.cli_surface.commands.len(),
        3,
        "expected 3 CRs in cli_surface.commands, got {:?}",
        enrichment.cli_surface.commands,
    );
    let cmd_set: Vec<&str> = enrichment
        .cli_surface
        .commands
        .iter()
        .map(|c| c.command.as_str())
        .collect();
    for expected in [
        "dec verify graph new",
        "dec verify graph run",
        "dec sparql query",
    ] {
        assert!(
            cmd_set.iter().any(|c| *c == expected),
            "cli_surface.commands missing {expected}; got {cmd_set:?}",
        );
    }
    assert_eq!(enrichment.cli_surface.capability_version, "0.3.0");
    assert_eq!(
        enrichment.cli_surface.dec_subcommands.len(),
        3,
        "dec_subcommands should mirror commands: {:?}",
        enrichment.cli_surface.dec_subcommands,
    );

    // ontology_vocabulary — namespace + classes.
    assert_eq!(
        enrichment.ontology_vocabulary.namespace,
        "https://decision-cli.dev/ns#"
    );
    for expected_class in ["VerificationGraph", "VerificationStep", "Session"] {
        assert!(
            enrichment
                .ontology_vocabulary
                .classes
                .iter()
                .any(|c| c == expected_class),
            "ontology_vocabulary.classes missing {expected_class}; got {:?}",
            enrichment.ontology_vocabulary.classes,
        );
    }
    assert_eq!(enrichment.ontology_vocabulary.prefix, "dec");
    assert_eq!(enrichment.ontology_vocabulary.source_od, "OD-001");

    // store_query_surface — local-oxigraph.
    assert_eq!(enrichment.store_query_surface.kind, "local-oxigraph");
    assert!(
        enrichment
            .store_query_surface
            .query_command
            .contains("dec sparql query"),
        "query_command should reference dec sparql query, got {}",
        enrichment.store_query_surface.query_command,
    );

    // env_capabilities — read from the env's concreteCapabilities block.
    assert_eq!(
        enrichment.env_capabilities.binaries_on_path,
        vec!["dec".to_string(), "bash".to_string()]
    );
    assert_eq!(
        enrichment.env_capabilities.writable_paths,
        vec!["$DEC_VERIFY_TMP".to_string(), "./".to_string()]
    );
    assert_eq!(
        enrichment.env_capabilities.environment_variables,
        vec!["DEC_VERIFY_TMP".to_string(), "PATH".to_string()]
    );

    // exemplar_graphs — both EX-001 and EX-002 with rationale + pattern.
    assert_eq!(enrichment.exemplar_graphs.len(), 2);
    let ex_ids: Vec<&str> = enrichment
        .exemplar_graphs
        .iter()
        .map(|e| e.id.as_str())
        .collect();
    assert!(ex_ids.contains(&"EX-001"));
    assert!(ex_ids.contains(&"EX-002"));
    for ex in &enrichment.exemplar_graphs {
        assert!(!ex.pattern_name.is_empty());
        assert!(!ex.rationale.is_empty());
        assert!(ex.exemplar_of.starts_with("https://decision-cli.dev/"));
    }

    // bundle_metadata.catalog_hashes — non-empty, one per CR/OD/EX.
    let hash_ids: Vec<&str> = enrichment
        .bundle_metadata
        .catalog_hashes
        .iter()
        .map(|e| e.id.as_str())
        .collect();
    for expected in ["CR-001", "CR-002", "CR-003", "OD-001", "EX-001", "EX-002"] {
        assert!(
            hash_ids.contains(&expected),
            "metadata.catalog_hashes missing {expected}; got {hash_ids:?}",
        );
    }
}

#[test]
fn tc_168_scenario_b_env_without_concrete_capabilities_falls_back() {
    let wd = WorkdirGuard::new("scenario-b");
    let seed = write_seed_definition(wd.path());
    init_run(wd.path(), DefinitionSource::File(seed)).expect("dec init");

    seed_catalog(wd.path());
    let _env_path = write_env_without_concrete(wd.path(), "ENV-002");
    let env = decision_cli::core::ontology::verification_env::from_turtle(
        &wd.path().join(".dec/verify/env/ENV-002.ttl"),
    )
    .expect("load env");

    let enrichment = assemble_enrichment_for(wd.path(), Some(&env), "ENV-002")
        .expect("assemble enrichment");

    // env_capabilities populated from the ephemeral-tempdir default.
    assert!(
        enrichment
            .env_capabilities
            .binaries_on_path
            .iter()
            .any(|b| b == "dec"),
        "default fallback should include 'dec'; got {:?}",
        enrichment.env_capabilities.binaries_on_path,
    );

    // Warnings includes the fallback notice.
    let joined = enrichment.bundle_metadata.warnings.join(" | ");
    assert!(
        joined.contains("ENV-002") && joined.contains("concreteCapabilities"),
        "warnings should mention env id + concreteCapabilities; got {joined:?}",
    );

    // The other four fields are still populated.
    assert!(!enrichment.cli_surface.commands.is_empty());
    assert!(!enrichment.ontology_vocabulary.namespace.is_empty());
    assert!(!enrichment.store_query_surface.kind.is_empty());
}

#[test]
fn tc_168_scenario_d_replay_determinism_via_bundle_hash() {
    let wd = WorkdirGuard::new("scenario-d");
    let seed = write_seed_definition(wd.path());
    init_run(wd.path(), DefinitionSource::File(seed)).expect("dec init");

    seed_catalog(wd.path());
    let _env_path = write_env_with_concrete(wd.path(), "ENV-001");
    let env = decision_cli::core::ontology::verification_env::from_turtle(
        &wd.path().join(".dec/verify/env/ENV-001.ttl"),
    )
    .expect("load env");

    let first = assemble_enrichment_for(wd.path(), Some(&env), "ENV-001").expect("first");
    let second = assemble_enrichment_for(wd.path(), Some(&env), "ENV-001").expect("second");

    // Same catalog state ⇒ same enrichment ⇒ same metadata hashes.
    assert_eq!(
        first.bundle_metadata.catalog_hashes,
        second.bundle_metadata.catalog_hashes,
        "catalog hashes must be deterministic across replays"
    );
    assert_eq!(first, second, "full enrichment must be deterministic");
}
