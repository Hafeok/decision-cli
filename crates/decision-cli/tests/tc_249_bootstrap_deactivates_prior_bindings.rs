//! TC-249 — Bootstrap deactivates prior role-binding versions in the same
//! transaction as new active write (invariant test for FT-118).
//!
//! Validates:
//! - When bootstrap writes a new active binding for role R, all other active
//!   bindings for R are atomically deactivated in the same transaction
//! - The uniqueness invariant (at most one active binding per role) holds
//!   at every observable moment
//! - Prior binding history is preserved (only dec:active flips)

use std::env;
use std::fs;
use std::path::PathBuf;

use decision_cli::core::bootstrap::bootstrap_catalog;
use decision_cli::core::ontology::role_binding::all_for_role;
use decision_cli::init::{run as init_run, DefinitionSource};
use oxigraph::io::RdfFormat;
use oxigraph::model::{Literal, NamedNode, Quad};
use oxigraph::store::Store;

#[test]
fn tc_249_bootstrap_deactivates_prior_bindings() {
    let workdir = tempdir("tc-249-bootstrap-deactivates");

    // 1. Initialize a workdir
    init_run(
        &workdir,
        DefinitionSource::Template("engineering-development".into()),
    )
    .expect("dec init template");

    let store = load_store_from_dump(&workdir);

    // 2. Inject a v1 binding for test-role (active)
    let bind_graph = NamedNode::new_unchecked("https://decision-cli.dev/ns/graph/role-binding");
    let v1_subject = NamedNode::new_unchecked("https://decision-cli.dev/ns/binding/test-role/v1");
    let rdf_type = NamedNode::new_unchecked("http://www.w3.org/1999/02/22-rdf-syntax-ns#type");
    let role_binding_class = NamedNode::new_unchecked("https://decision-cli.dev/ns#RoleBinding");
    let role_id_pred = NamedNode::new_unchecked("https://decision-cli.dev/ns#role_id");
    let default_cap_pred =
        NamedNode::new_unchecked("https://decision-cli.dev/ns#default_capability");
    let active_pred = NamedNode::new_unchecked("https://decision-cli.dev/ns#active");
    let version_pred = NamedNode::new_unchecked("https://decision-cli.dev/ns#version");
    let escalation_steps_pred =
        NamedNode::new_unchecked("https://decision-cli.dev/ns#escalation_steps");
    let rdf_nil = NamedNode::new_unchecked("http://www.w3.org/1999/02/22-rdf-syntax-ns#nil");
    let cap_v1 = NamedNode::new_unchecked("https://decision-cli.dev/ns/capability/code-writer/v1");
    let xsd_bool = NamedNode::new_unchecked("http://www.w3.org/2001/XMLSchema#boolean");
    let xsd_int = NamedNode::new_unchecked("http://www.w3.org/2001/XMLSchema#integer");

    let v1_quads = vec![
        Quad::new(
            v1_subject.clone(),
            rdf_type.clone(),
            role_binding_class.clone(),
            bind_graph.clone(),
        ),
        Quad::new(
            v1_subject.clone(),
            role_id_pred.clone(),
            Literal::new_simple_literal("test-role"),
            bind_graph.clone(),
        ),
        Quad::new(
            v1_subject.clone(),
            default_cap_pred.clone(),
            cap_v1.clone(),
            bind_graph.clone(),
        ),
        Quad::new(
            v1_subject.clone(),
            active_pred.clone(),
            Literal::new_typed_literal("true", xsd_bool.clone()),
            bind_graph.clone(),
        ),
        Quad::new(
            v1_subject.clone(),
            version_pred.clone(),
            Literal::new_typed_literal("1", xsd_int.clone()),
            bind_graph.clone(),
        ),
        Quad::new(
            v1_subject.clone(),
            escalation_steps_pred.clone(),
            rdf_nil.clone(),
            bind_graph.clone(),
        ),
    ];

    store
        .transaction(|mut tx| {
            for q in &v1_quads {
                tx.insert(q.as_ref())?;
            }
            Ok::<_, oxigraph::store::StorageError>(())
        })
        .expect("inject v1 binding");

    // Persist the store with v1 active
    persist_store(&store, &workdir);

    // 3. Verify v1 is active
    let bindings = all_for_role(&store, "test-role").expect("read bindings");
    assert_eq!(bindings.len(), 1, "should have one binding");
    assert_eq!(bindings[0].version, 1);
    assert!(bindings[0].active, "v1 should be active before bootstrap");

    // 4. Create YAML files with v7 binding
    let config_dir = workdir.join("config");
    fs::create_dir_all(&config_dir).expect("create config dir");

    let cap_yaml = config_dir.join("capabilities.yaml");
    fs::write(
        &cap_yaml,
        r#"capabilities:
  - capability_id: code-writer
    version: 1
    endpoint: anthropic
    model_identifier: claude-sonnet-4-20250514
    tier: 1
    context_window: 200000
    max_output: 16000
    supports_vision: true
    supports_tool_calling: true
    cost_input_per_m: "3.00"
    cost_output_per_m: "15.00"
    cost_currency: USD
    status: active
  - capability_id: code-writer
    version: 2
    endpoint: anthropic
    model_identifier: claude-sonnet-4.5-20250610
    tier: 1
    context_window: 200000
    max_output: 16000
    supports_vision: true
    supports_tool_calling: true
    cost_input_per_m: "3.00"
    cost_output_per_m: "15.00"
    cost_currency: USD
    status: active
"#,
    )
    .expect("write capabilities.yaml");

    let bind_yaml = config_dir.join("role-bindings.yaml");
    fs::write(
        &bind_yaml,
        r#"role_bindings:
  - role_id: test-role
    version: 7
    default_capability: code-writer
    escalation_steps: []
    active: true
"#,
    )
    .expect("write role-bindings.yaml");

    // 5. Run bootstrap (should deactivate v1, activate v7)
    let report = bootstrap_catalog(&workdir, &cap_yaml, &bind_yaml, false, false)
        .expect("bootstrap should succeed");

    assert_eq!(
        report.bindings_written, 1,
        "should write one new binding (v7)"
    );

    // 6. Reload the store and verify the invariant
    let store_after = load_store_from_dump(&workdir);
    let bindings_after = all_for_role(&store_after, "test-role").expect("read bindings after");

    assert_eq!(bindings_after.len(), 2, "should have two bindings total");

    let v1_after = bindings_after
        .iter()
        .find(|b| b.version == 1)
        .expect("v1 should still exist");
    let v7_after = bindings_after
        .iter()
        .find(|b| b.version == 7)
        .expect("v7 should exist");

    assert!(!v1_after.active, "v1 should be inactive after bootstrap");
    assert!(v7_after.active, "v7 should be active after bootstrap");

    // 7. Verify exactly one active binding for the role
    let active_count = bindings_after.iter().filter(|b| b.active).count();
    assert_eq!(
        active_count, 1,
        "exactly one active binding should exist (uniqueness invariant)"
    );

    // 8. Verify v1 history is preserved (default_capability unchanged)
    assert_eq!(
        v1_after.default_capability.as_str(),
        cap_v1.as_str(),
        "v1's default_capability should be preserved"
    );
}

fn load_store_from_dump(workdir: &PathBuf) -> Store {
    let dump = workdir.join(".dec").join("store").join("orchestration.nq");
    let bytes = fs::read(&dump).expect("read store dump");
    let store = Store::new().expect("store");
    store
        .load_from_reader(RdfFormat::NQuads, bytes.as_slice())
        .expect("load store");
    store
}

fn persist_store(store: &Store, workdir: &PathBuf) {
    let dump = workdir.join(".dec").join("store").join("orchestration.nq");
    let mut buf = Vec::new();
    store
        .dump_to_writer(RdfFormat::NQuads, &mut buf)
        .expect("dump store");
    fs::write(&dump, buf).expect("write store dump");
}

fn tempdir(label: &str) -> PathBuf {
    let mut p = env::temp_dir();
    let pid = std::process::id();
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    p.push(format!("{label}-{pid}-{nonce}"));
    fs::create_dir_all(&p).expect("create tempdir");
    p
}
