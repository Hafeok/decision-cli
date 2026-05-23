//! TC-059 — StreamWriter invokes safety check on every graph and step commit.
//!
//! Validates: FT-037 · ADR-028.
//! Spec: `.product/tests/TC-059-streamwriter-invokes-safety-check-on-every-graph-a.md`
//!
//! Five acceptance criteria, one `#[test]` each. The structural test
//! for §Acceptance 4 (`no_safety_bypass`) lives separately under
//! `tests/structural/`.

use std::sync::Arc;

use decision_cli::core::ontology::verification_env::{SafetyClass, VerificationEnvironment};
use decision_cli::core::ontology::verification_graph::{
    ArtifactRef, StepFields, VerificationGraph, VerificationStep,
};
use decision_cli::vocab::{verify_env_graph, verify_graph_named_graph};
use decision_cli::StreamWriter;
use oxi_events::Mutation;
use oxigraph::model::{NamedNode, Quad};
use oxigraph::store::Store;

const STREAM_IRI: &str = "https://decision-cli.dev/stream/tc-059";

fn writer() -> (Arc<Store>, StreamWriter) {
    let store = Arc::new(Store::new().expect("in-memory store"));
    let stream = NamedNode::new(STREAM_IRI).expect("stream iri");
    let w = StreamWriter::bootstrap(Arc::clone(&store), stream).expect("stream writer");
    (store, w)
}

fn ephemeral_env() -> VerificationEnvironment {
    VerificationEnvironment {
        id: "ENV-001-ephemeral-cli".to_string(),
        env_type: "ephemeral-tempdir".to_string(),
        setup: Some("mkdir -p $TMPDIR".to_string()),
        teardown: Some("rm -rf $TMPDIR".to_string()),
        allowed_ops: vec![
            "shell".to_string(),
            "filesystem".to_string(),
            "sparql-local".to_string(),
        ],
        safety_class: SafetyClass::Isolated,
        endpoint: None,
        fixture_source: None,
    }
}

fn prod_readonly_env() -> VerificationEnvironment {
    VerificationEnvironment {
        id: "ENV-002-prod-readonly".to_string(),
        env_type: "remote-http".to_string(),
        setup: None,
        teardown: None,
        allowed_ops: vec!["http-readonly".to_string()],
        safety_class: SafetyClass::ProductionReadonly,
        endpoint: Some("https://decision-cli.dev".to_string()),
        fixture_source: None,
    }
}

fn ft_001() -> ArtifactRef {
    ArtifactRef(NamedNode::new_unchecked(
        "https://decision-cli.dev/ns/feature/FT-001",
    ))
}

fn commit_quads(w: &StreamWriter, quads: Vec<Quad>) -> Result<(), String> {
    w.commit(Mutation::insert(quads))
        .map(|_| ())
        .map_err(|e| format!("{e:#}"))
}

fn http_post_step(graph: &str, idx: usize) -> VerificationStep {
    VerificationStep::new(
        graph,
        idx,
        StepFields::HttpRequest {
            method: "POST".to_string(),
            url: "https://example.com".to_string(),
            expect_status: Some(200),
        },
    )
}

fn shell_step(graph: &str, idx: usize) -> VerificationStep {
    VerificationStep::new(
        graph,
        idx,
        StepFields::ShellCommand {
            command: "true".to_string(),
            expect_exit_code: Some(0),
            capture_output: None,
        },
    )
}

#[test]
fn graph_commit_invokes_graph_level_safety_check() {
    // (1) A graph referencing prod-readonly env, containing an http-POST
    // step, must be aborted before SHACL runs.
    let (store, w) = writer();
    // Seed env in store first.
    let env = prod_readonly_env();
    commit_quads(&w, env.to_quads(verify_env_graph())).expect("env commits cleanly");

    // Build a graph that references env and has an http-POST step.
    let g = VerificationGraph::new(
        "VG-tc059-a",
        ft_001(),
        env.iri(),
        vec![http_post_step("VG-tc059-a", 0)],
    );
    let graph_quads = g.to_quads(verify_graph_named_graph());
    let err = commit_quads(&w, graph_quads.clone()).expect_err("safety abort");
    assert!(
        err.contains("safety violation"),
        "expected 'safety violation' in {err}"
    );
    // The error must name the offending op.
    assert!(
        err.contains("http-mutating"),
        "expected `http-mutating` in {err}"
    );

    // The graph quads must NOT have been written.
    let graph_iri = g.id.clone();
    let touched = store
        .quads_for_pattern(
            Some(oxigraph::model::Subject::NamedNode(graph_iri).as_ref()),
            None,
            None,
            None,
        )
        .next()
        .is_some();
    assert!(!touched, "graph must NOT persist when safety rejects");
}

#[test]
fn step_append_invokes_per_step_safety_check() {
    // (2) Commit a graph against prod-readonly first (with an empty
    // safe step), then attempt to append a violating step. The
    // append must abort.
    let (store, w) = writer();
    let env = prod_readonly_env();
    commit_quads(&w, env.to_quads(verify_env_graph())).expect("env commits");

    // Original graph: empty steps so it commits cleanly.
    let g = VerificationGraph::new("VG-tc059-b", ft_001(), env.iri(), vec![]);
    commit_quads(&w, g.to_quads(verify_graph_named_graph())).expect("empty graph commits");

    // Append the violating step — atomic rewrite means full graph re-emit.
    let g_with_step = VerificationGraph::new(
        "VG-tc059-b",
        ft_001(),
        env.iri(),
        vec![http_post_step("VG-tc059-b", 0)],
    );
    let err = commit_quads(&w, g_with_step.to_quads(verify_graph_named_graph()))
        .expect_err("safety must abort the append");
    assert!(err.contains("safety violation"), "{err}");

    // No step quads persisted for VG-tc059-b/0.
    let step_iri = decision_cli::core::ontology::verification_graph::step_iri_for("VG-tc059-b", 0);
    let touched = store
        .quads_for_pattern(
            Some(oxigraph::model::Subject::NamedNode(step_iri).as_ref()),
            None,
            None,
            None,
        )
        .next()
        .is_some();
    assert!(!touched, "violating step must NOT persist");
}

#[test]
fn empty_graph_passes_trivially_through_writer() {
    // (3) A new graph with `dec:steps ()` against any env passes
    // unconditionally — even against prod-readonly.
    let (_store, w) = writer();
    let env = prod_readonly_env();
    commit_quads(&w, env.to_quads(verify_env_graph())).expect("env commits");

    let empty = VerificationGraph::new("VG-tc059-c", ft_001(), env.iri(), vec![]);
    commit_quads(&w, empty.to_quads(verify_graph_named_graph()))
        .expect("empty graph commits cleanly against prod-readonly env");
}

#[test]
fn safety_runs_before_shacl_distinct_error() {
    // (5) A graph that violates both SHACL and safety surfaces safety
    // first (per FT-037 §Behaviour step 4 — safety check runs before
    // SHACL pass). Both error variants must exist and be reachable.
    let (_store, w) = writer();
    let env = prod_readonly_env();
    commit_quads(&w, env.to_quads(verify_env_graph())).expect("env commits");

    // Graph violating safety (http-POST against prod-readonly) AND
    // valid SHACL (well-formed graph + step structure).
    let g = VerificationGraph::new(
        "VG-tc059-d",
        ft_001(),
        env.iri(),
        vec![http_post_step("VG-tc059-d", 0)],
    );
    let err = commit_quads(&w, g.to_quads(verify_graph_named_graph()))
        .expect_err("safety violation aborts");
    assert!(err.contains("safety violation"), "{err}");

    // Separately: a graph with a malformed step body produces SHACL
    // violation rather than safety violation. Build a graph against
    // the *isolated* env (so safety would pass), but with a step
    // missing `dec:command` — SHACL must catch it.
    let iso_env = ephemeral_env();
    commit_quads(&w, iso_env.to_quads(verify_env_graph())).expect("iso env commits");

    let mut malformed = VerificationGraph::new(
        "VG-tc059-e",
        ft_001(),
        iso_env.iri(),
        vec![shell_step("VG-tc059-e", 0)],
    )
    .to_quads(verify_graph_named_graph());
    // Strip the `dec:command` literal so the SHACL shape catches the
    // shell-command field-missing case.
    malformed.retain(|q| q.predicate.as_str() != "https://decision-cli.dev/ns#command");
    let shacl_err = commit_quads(&w, malformed).expect_err("SHACL rejects malformed step");
    assert!(
        shacl_err.contains("SHACL violation"),
        "expected SHACL prefix; got {shacl_err}"
    );
    // The two errors are distinct (separate prefixes).
    assert!(
        !shacl_err.contains("safety violation"),
        "shacl_err: {shacl_err}"
    );
}

/// TC-059 §Acceptance 4 — structural / grep-based assertion that
/// `verification_graph::*` types are never inserted into the store
/// outside the `StreamWriter::commit` path. Feature code must not call
/// `store.insert` / `store.bulk_loader().*` with verification-graph
/// quads on its own — every mutation routes through `StreamWriter`.
#[test]
fn no_safety_bypass_in_features() {
    use std::fs;
    use std::path::{Path, PathBuf};

    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        let Ok(entries) = fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                walk(&p, out);
            } else if p.extension().and_then(|s| s.to_str()) == Some("rs") {
                out.push(p);
            }
        }
    }

    let features_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join("features");
    let mut files = Vec::new();
    walk(&features_root, &mut files);
    assert!(
        !files.is_empty(),
        "expected feature .rs files under {features_root:?}"
    );

    let mut offenders: Vec<String> = Vec::new();
    for f in files {
        let Ok(content) = fs::read_to_string(&f) else {
            continue;
        };
        // Strip `#[cfg(test)] mod tests { ... }` so test fixtures that
        // legitimately seed quads to exercise read-side helpers don't
        // register as production-code bypasses. Unit-test code is not
        // a feature-code execution path.
        let prod_content = strip_cfg_test_blocks(&content);
        // Feature code must not call `Store::insert` / bulk_loader on
        // anything that came from `verification_graph::*` types. We
        // detect the smell by flagging files that *both* import any
        // verification_graph symbol AND call store.insert / bulk_loader.
        let imports_vg = prod_content.contains("verification_graph::")
            || prod_content.contains("VerificationGraph")
            || prod_content.contains("VerificationStep");
        if !imports_vg {
            continue;
        }
        // Look for a raw store mutation method.
        let mutating_paths = [".insert(", ".bulk_loader(", ".bulk_extend("];
        for needle in mutating_paths {
            if prod_content.contains(needle) {
                offenders.push(format!("{}: '{needle}'", f.display()));
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "feature code must not bypass StreamWriter when handling \
         verification-graph quads; offenders: {offenders:?}"
    );
}

/// Remove every `#[cfg(test)] mod NAME { ... }` block from a Rust
/// source string by walking brace depth from each occurrence of the
/// attribute. Returns the surviving (production-code) text. Operates
/// on byte indices (safe because `{`, `}`, and the needle are ASCII)
/// and slices the original `&str` to preserve UTF-8 boundaries.
fn strip_cfg_test_blocks(src: &str) -> String {
    const NEEDLE: &str = "#[cfg(test)]";
    let bytes = src.as_bytes();
    let mut out = String::with_capacity(src.len());
    let mut cursor = 0;
    while let Some(rel) = src[cursor..].find(NEEDLE) {
        let start = cursor + rel;
        out.push_str(&src[cursor..start]);
        let mut j = start + NEEDLE.len();
        while j < bytes.len() && bytes[j] != b'{' {
            j += 1;
        }
        if j == bytes.len() {
            cursor = bytes.len();
            break;
        }
        let mut depth = 1i32;
        j += 1;
        while j < bytes.len() && depth > 0 {
            match bytes[j] {
                b'{' => depth += 1,
                b'}' => depth -= 1,
                _ => {}
            }
            j += 1;
        }
        cursor = j;
    }
    out.push_str(&src[cursor..]);
    out
}

#[test]
fn safety_check_aborts_before_shacl_runs() {
    // Cross-check: a graph with BOTH a SHACL violation AND a safety
    // violation surfaces the safety error first (safety runs before
    // SHACL per FT-037 §Behaviour). Verifies ordering, not just
    // independence.
    let (_store, w) = writer();
    let env = prod_readonly_env();
    commit_quads(&w, env.to_quads(verify_env_graph())).expect("env commits");

    // Build a graph: http-POST (violates safety) AND missing
    // dec:method (would violate SHACL). Safety must trigger first.
    let mut quads = VerificationGraph::new(
        "VG-tc059-f",
        ft_001(),
        env.iri(),
        vec![http_post_step("VG-tc059-f", 0)],
    )
    .to_quads(verify_graph_named_graph());
    // Strip method to also trigger SHACL.
    quads.retain(|q| q.predicate.as_str() != "https://decision-cli.dev/ns#method");
    let err = commit_quads(&w, quads).expect_err("must abort");
    assert!(
        err.contains("safety violation"),
        "expected safety violation prefix to win the race; got {err}"
    );
}
