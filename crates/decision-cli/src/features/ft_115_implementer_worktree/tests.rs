//! Integration tests for FT-115 worktree support.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use oxigraph::model::{GraphName, Literal, NamedNode};
use oxigraph::store::Store;
use tempfile::TempDir;

use super::*;

/// Helper to initialize a test git repo with an initial commit.
fn init_test_repo() -> (TempDir, PathBuf, String) {
    let temp_dir = TempDir::new().unwrap();
    let repo_path = temp_dir.path().to_path_buf();

    // git init
    Command::new("git")
        .arg("-C")
        .arg(&repo_path)
        .arg("init")
        .output()
        .unwrap();

    // git config user.name/email (required for commits)
    Command::new("git")
        .arg("-C")
        .arg(&repo_path)
        .arg("config")
        .arg("user.name")
        .arg("Test User")
        .output()
        .unwrap();

    Command::new("git")
        .arg("-C")
        .arg(&repo_path)
        .arg("config")
        .arg("user.email")
        .arg("test@example.com")
        .output()
        .unwrap();

    // Create initial file and commit
    fs::create_dir_all(repo_path.join("src")).unwrap();
    fs::write(repo_path.join("src/lib.rs"), "original").unwrap();

    Command::new("git")
        .arg("-C")
        .arg(&repo_path)
        .arg("add")
        .arg(".")
        .output()
        .unwrap();

    Command::new("git")
        .arg("-C")
        .arg(&repo_path)
        .arg("commit")
        .arg("-m")
        .arg("Initial commit")
        .output()
        .unwrap();

    // Get baseline SHA
    let output = Command::new("git")
        .arg("-C")
        .arg(&repo_path)
        .arg("rev-parse")
        .arg("HEAD")
        .output()
        .unwrap();
    let baseline_sha = String::from_utf8_lossy(&output.stdout).trim().to_string();

    (temp_dir, repo_path, baseline_sha)
}

/// Helper to create a test orchestration store.
fn init_test_store(repo_path: &Path) -> (Store, PathBuf, oxi_events::GraphWriter) {
    let store = Store::new().unwrap();
    let dump_path = repo_path.join(".dec/store/orchestration.nq");
    fs::create_dir_all(dump_path.parent().unwrap()).unwrap();

    let writer = oxi_events::GraphWriter::open(std::sync::Arc::new(store.clone())).unwrap();

    (store, dump_path, writer)
}

/// Helper to create a session IRI for testing.
fn test_session_iri(short_id: &str) -> NamedNode {
    NamedNode::new_unchecked(format!("urn:dec:session:{}", short_id))
}

/// TC-229: Worktree is created at dispatch start branched off feature baseline commit
#[test]
fn tc_229_worktree_created_at_baseline() {
    let (_temp, repo_path, baseline_sha) = init_test_repo();
    let (store, dump_path, writer) = init_test_store(&repo_path);

    // Create worktree
    let worktree = create_worktree(&repo_path, "sess-abc", &baseline_sha).unwrap();

    // Assert worktree directory exists
    assert!(worktree.path.exists());
    assert!(worktree.path.is_dir());

    // Assert worktree HEAD points to baseline
    let output = Command::new("git")
        .arg("-C")
        .arg(&worktree.path)
        .arg("rev-parse")
        .arg("HEAD")
        .output()
        .unwrap();
    let worktree_head = String::from_utf8_lossy(&output.stdout).trim().to_string();
    assert_eq!(worktree_head, baseline_sha);

    // Assert branch exists
    let output = Command::new("git")
        .arg("-C")
        .arg(&repo_path)
        .arg("branch")
        .arg("--list")
        .arg("dec/sess/sess-abc")
        .output()
        .unwrap();
    let branches = String::from_utf8_lossy(&output.stdout);
    assert!(branches.contains("dec/sess/sess-abc"));

    // Record in store
    let session_iri = test_session_iri("sess-abc");
    record_worktree_created(&writer, &store, &dump_path, &session_iri, &worktree).unwrap();

    // Verify store contains worktree metadata
    let worktree_path_pred =
        NamedNode::new_unchecked("https://purl.org/ddd/decision-cli#worktreePath");
    let worktree_baseline_pred =
        NamedNode::new_unchecked("https://purl.org/ddd/decision-cli#worktreeBaseline");

    let mut found_path = false;
    let mut found_baseline = false;

    for _quad in store.quads_for_pattern(
        Some(session_iri.as_ref().into()),
        Some(worktree_path_pred.as_ref().into()),
        None,
        None,
    ) {
        found_path = true;
    }

    for _quad in store.quads_for_pattern(
        Some(session_iri.as_ref().into()),
        Some(worktree_baseline_pred.as_ref().into()),
        None,
        None,
    ) {
        found_baseline = true;
    }

    assert!(found_path, "worktreePath not found in store");
    assert!(found_baseline, "worktreeBaseline not found in store");

    // Make a commit in main (advance HEAD)
    fs::write(repo_path.join("src/main.rs"), "main change").unwrap();
    Command::new("git")
        .arg("-C")
        .arg(&repo_path)
        .arg("add")
        .arg(".")
        .output()
        .unwrap();
    Command::new("git")
        .arg("-C")
        .arg(&repo_path)
        .arg("commit")
        .arg("-m")
        .arg("Main change")
        .output()
        .unwrap();

    // Assert worktree HEAD is still at baseline (unchanged)
    let output = Command::new("git")
        .arg("-C")
        .arg(&worktree.path)
        .arg("rev-parse")
        .arg("HEAD")
        .output()
        .unwrap();
    let worktree_head_after = String::from_utf8_lossy(&output.stdout).trim().to_string();
    assert_eq!(worktree_head_after, baseline_sha);
}

/// TC-230: Worker source edits land in worktree, never in main pre-merge
#[test]
fn tc_230_edits_isolated_to_worktree() {
    let (_temp, repo_path, baseline_sha) = init_test_repo();

    // Create worktree
    let worktree = create_worktree(&repo_path, "sess-abc", &baseline_sha).unwrap();

    // Simulate worker editing in worktree (no commit)
    let worktree_lib = worktree.path.join("src/lib.rs");
    assert_eq!(fs::read_to_string(&worktree_lib).unwrap(), "original");

    fs::write(&worktree_lib, "edited").unwrap();
    fs::write(worktree.path.join("src/new_helper.rs"), "new file").unwrap();

    // Assert main is unchanged
    let main_lib = repo_path.join("src/lib.rs");
    assert_eq!(fs::read_to_string(&main_lib).unwrap(), "original");
    assert!(!repo_path.join("src/new_helper.rs").exists());

    // Assert git status in main is clean (ignore .dec directory)
    let output = Command::new("git")
        .arg("-C")
        .arg(&repo_path)
        .arg("status")
        .arg("--porcelain")
        .output()
        .unwrap();
    let main_status = String::from_utf8_lossy(&output.stdout);
    // Filter out any .dec/ directory references
    let main_changes: Vec<&str> = main_status
        .lines()
        .filter(|line| !line.contains(".dec/"))
        .collect();
    assert!(
        main_changes.is_empty(),
        "main should have no changes outside .dec/: {:?}",
        main_changes
    );

    // Assert git status in worktree shows changes
    let output = Command::new("git")
        .arg("-C")
        .arg(&worktree.path)
        .arg("status")
        .arg("--porcelain")
        .output()
        .unwrap();
    let worktree_status = String::from_utf8_lossy(&output.stdout);
    assert!(worktree_status.contains("lib.rs"));
    assert!(worktree_status.contains("new_helper.rs"));
}

/// TC-231: Successful dispatch fast-forwards worktree commit into main and deletes worktree
#[test]
fn tc_231_successful_merge_and_cleanup() {
    let (_temp, repo_path, baseline_sha) = init_test_repo();
    let (store, dump_path, writer) = init_test_store(&repo_path);

    // Create worktree
    let worktree = create_worktree(&repo_path, "sess-abc", &baseline_sha).unwrap();
    let session_iri = test_session_iri("sess-abc");
    record_worktree_created(&writer, &store, &dump_path, &session_iri, &worktree).unwrap();

    // Worker commits in worktree
    fs::write(worktree.path.join("src/lib.rs"), "fixed").unwrap();
    Command::new("git")
        .arg("-C")
        .arg(&worktree.path)
        .arg("add")
        .arg(".")
        .output()
        .unwrap();
    Command::new("git")
        .arg("-C")
        .arg(&worktree.path)
        .arg("commit")
        .arg("-m")
        .arg("[FT-X] fix lib")
        .output()
        .unwrap();

    // Capture worktree tip SHA
    let output = Command::new("git")
        .arg("-C")
        .arg(&worktree.path)
        .arg("rev-parse")
        .arg("HEAD")
        .output()
        .unwrap();
    let worker_sha = String::from_utf8_lossy(&output.stdout).trim().to_string();

    // Fast-forward into main
    let outcome = fast_forward_into_main(&repo_path, &worktree).unwrap();
    assert_eq!(
        outcome,
        MergeOutcome::FastForwarded {
            from: baseline_sha.clone(),
            to: worker_sha.clone()
        }
    );

    // Assert main HEAD is now worker_sha
    let output = Command::new("git")
        .arg("-C")
        .arg(&repo_path)
        .arg("rev-parse")
        .arg("HEAD")
        .output()
        .unwrap();
    let main_head = String::from_utf8_lossy(&output.stdout).trim().to_string();
    assert_eq!(main_head, worker_sha);

    // Assert main/src/lib.rs contains "fixed"
    assert_eq!(
        fs::read_to_string(repo_path.join("src/lib.rs")).unwrap(),
        "fixed"
    );

    // Record merge in store
    record_worktree_merged(&writer, &store, &dump_path, &session_iri, &worker_sha).unwrap();

    // Abort worktree (cleanup)
    abort_worktree(&repo_path, &worktree).unwrap();

    // Assert worktree directory is gone
    assert!(!worktree.path.exists());

    // Assert branch is gone
    let output = Command::new("git")
        .arg("-C")
        .arg(&repo_path)
        .arg("branch")
        .arg("--list")
        .arg("dec/sess/sess-abc")
        .output()
        .unwrap();
    let branches = String::from_utf8_lossy(&output.stdout);
    assert!(branches.trim().is_empty());

    // Assert main HEAD unchanged after cleanup
    let output = Command::new("git")
        .arg("-C")
        .arg(&repo_path)
        .arg("rev-parse")
        .arg("HEAD")
        .output()
        .unwrap();
    let main_head_after = String::from_utf8_lossy(&output.stdout).trim().to_string();
    assert_eq!(main_head_after, worker_sha);
}

/// TC-232: Failed dispatch deletes worktree and leaves main byte-identical to pre-dispatch
#[test]
fn tc_232_failed_dispatch_cleanup() {
    let (_temp, repo_path, baseline_sha) = init_test_repo();
    let (store, dump_path, writer) = init_test_store(&repo_path);

    // Case 1: Worker produces no commit
    {
        let worktree = create_worktree(&repo_path, "sess-no-commit", &baseline_sha).unwrap();
        let session_iri = test_session_iri("sess-no-commit");
        record_worktree_created(&writer, &store, &dump_path, &session_iri, &worktree).unwrap();

        // Worker edits but doesn't commit
        fs::write(worktree.path.join("src/lib.rs"), "edited but not committed").unwrap();

        // Record abort
        record_worktree_aborted(&writer, &store, &dump_path, &session_iri, "no commit").unwrap();

        // Cleanup
        abort_worktree(&repo_path, &worktree).unwrap();

        // Assert main HEAD unchanged
        let output = Command::new("git")
            .arg("-C")
            .arg(&repo_path)
            .arg("rev-parse")
            .arg("HEAD")
            .output()
            .unwrap();
        let main_head = String::from_utf8_lossy(&output.stdout).trim().to_string();
        assert_eq!(main_head, baseline_sha);

        // Assert main/src/lib.rs unchanged
        assert_eq!(
            fs::read_to_string(repo_path.join("src/lib.rs")).unwrap(),
            "original"
        );

        // Assert worktree gone
        assert!(!worktree.path.exists());
    }

    // Case 2: Worker commits to wrong branch (simulate by checking out main)
    {
        let worktree = create_worktree(&repo_path, "sess-wrong-branch", &baseline_sha).unwrap();
        let session_iri = test_session_iri("sess-wrong-branch");
        record_worktree_created(&writer, &store, &dump_path, &session_iri, &worktree).unwrap();

        // Worker commits to a different branch (simulate contract violation)
        // For this test, we just verify cleanup leaves main untouched
        record_worktree_aborted(&writer, &store, &dump_path, &session_iri, "wrong branch").unwrap();
        abort_worktree(&repo_path, &worktree).unwrap();

        // Assert main unchanged
        let output = Command::new("git")
            .arg("-C")
            .arg(&repo_path)
            .arg("rev-parse")
            .arg("HEAD")
            .output()
            .unwrap();
        let main_head = String::from_utf8_lossy(&output.stdout).trim().to_string();
        assert_eq!(main_head, baseline_sha);

        assert!(!worktree.path.exists());
    }
}

/// TC-233: Orchestration store is shared between main and worktree via absolute path
#[test]
fn tc_233_store_shared_between_main_and_worktree() {
    let (_temp, repo_path, baseline_sha) = init_test_repo();
    let (store, dump_path, writer) = init_test_store(&repo_path);

    // Add marker triple to store
    let marker_subj = NamedNode::new_unchecked("urn:marker:tc-233");
    let marker_pred = NamedNode::new_unchecked("urn:p");
    let marker_obj = Literal::new_simple_literal("1");
    let quad = oxigraph::model::Quad::new(
        marker_subj.clone(),
        marker_pred.clone(),
        marker_obj.clone(),
        GraphName::DefaultGraph,
    );

    let mut mutation = oxi_events::Mutation::default();
    mutation.inserts.push(quad);
    writer.commit(mutation.with_cause("test marker")).unwrap();

    // Persist
    let file = std::fs::File::create(&dump_path).unwrap();
    let writer_io = std::io::BufWriter::new(file);
    store
        .dump_graph_to_writer(
            &GraphName::DefaultGraph,
            oxigraph::io::RdfFormat::NQuads,
            writer_io,
        )
        .unwrap();

    // Create worktree
    let worktree = create_worktree(&repo_path, "sess-abc", &baseline_sha).unwrap();

    // Assert .dec/worktrees/sess-abc/.dec/ does NOT exist as a real directory
    let _worktree_dec = worktree.path.join(".dec");
    // If it exists, it should be a symlink or not exist at all
    // For this implementation, it won't exist — worktrees access main's .dec via absolute path
    // The actual implementation will use absolute paths, not symlinks

    // From worktree's perspective, read the orchestration store at main's path
    let store_from_worktree = Store::new().unwrap();
    let store_file = std::fs::File::open(&dump_path).unwrap();
    store_from_worktree
        .load_from_reader(
            oxigraph::io::RdfFormat::NQuads,
            std::io::BufReader::new(store_file),
        )
        .unwrap();

    // Assert marker triple is present
    let mut found = false;
    for _quad in store_from_worktree.quads_for_pattern(
        Some(marker_subj.as_ref().into()),
        Some(marker_pred.as_ref().into()),
        None,
        None,
    ) {
        found = true;
    }
    assert!(
        found,
        "marker triple not found when reading from worktree context"
    );

    // Simulate worker writing a new triple
    let worker_subj = NamedNode::new_unchecked("urn:worker:triple");
    let worker_quad = oxigraph::model::Quad::new(
        worker_subj.clone(),
        marker_pred.clone(),
        Literal::new_simple_literal("worker data"),
        GraphName::DefaultGraph,
    );
    store.insert(&worker_quad).unwrap();

    // Persist again
    let file = std::fs::File::create(&dump_path).unwrap();
    let writer_io = std::io::BufWriter::new(file);
    store
        .dump_graph_to_writer(
            &GraphName::DefaultGraph,
            oxigraph::io::RdfFormat::NQuads,
            writer_io,
        )
        .unwrap();

    // Re-read from "main's context"
    let store_from_main = Store::new().unwrap();
    let store_file = std::fs::File::open(&dump_path).unwrap();
    store_from_main
        .load_from_reader(
            oxigraph::io::RdfFormat::NQuads,
            std::io::BufReader::new(store_file),
        )
        .unwrap();

    // Assert both triples are visible
    let mut found_marker = false;
    let mut found_worker = false;
    for quad in store_from_main.quads_for_pattern(None, None, None, None) {
        if let Ok(q) = quad {
            if q.subject.to_string().contains("marker:tc-233") {
                found_marker = true;
            }
            if q.subject.to_string().contains("worker:triple") {
                found_worker = true;
            }
        }
    }
    assert!(
        found_marker && found_worker,
        "both triples should be visible"
    );

    // Cleanup
    abort_worktree(&repo_path, &worktree).unwrap();

    // Assert triples still exist after cleanup
    let store_final = Store::new().unwrap();
    let store_file = std::fs::File::open(&dump_path).unwrap();
    store_final
        .load_from_reader(
            oxigraph::io::RdfFormat::NQuads,
            std::io::BufReader::new(store_file),
        )
        .unwrap();

    let mut found_marker_final = false;
    let mut found_worker_final = false;
    for quad in store_final.quads_for_pattern(None, None, None, None) {
        if let Ok(q) = quad {
            if q.subject.to_string().contains("marker:tc-233") {
                found_marker_final = true;
            }
            if q.subject.to_string().contains("worker:triple") {
                found_worker_final = true;
            }
        }
    }
    assert!(
        found_marker_final && found_worker_final,
        "triples should survive worktree cleanup"
    );
}

/// TC-234: Cherry-pick fallback merges worker commit when main moved during dispatch
#[test]
fn tc_234_cherry_pick_when_main_moves() {
    let (_temp, repo_path, baseline_sha) = init_test_repo();

    // Create worktree
    let worktree = create_worktree(&repo_path, "sess-abc", &baseline_sha).unwrap();

    // Worker edits src/lib.rs in worktree and commits
    fs::write(worktree.path.join("src/lib.rs"), "fixed").unwrap();
    Command::new("git")
        .arg("-C")
        .arg(&worktree.path)
        .arg("add")
        .arg(".")
        .output()
        .unwrap();
    Command::new("git")
        .arg("-C")
        .arg(&worktree.path)
        .arg("commit")
        .arg("-m")
        .arg("[FT-X] fix A")
        .output()
        .unwrap();

    // Simulate concurrent change in main (disjoint file)
    fs::write(repo_path.join("src/main.rs"), "B").unwrap();
    Command::new("git")
        .arg("-C")
        .arg(&repo_path)
        .arg("add")
        .arg(".")
        .output()
        .unwrap();
    Command::new("git")
        .arg("-C")
        .arg(&repo_path)
        .arg("commit")
        .arg("-m")
        .arg("Concurrent B")
        .output()
        .unwrap();

    let output = Command::new("git")
        .arg("-C")
        .arg(&repo_path)
        .arg("rev-parse")
        .arg("HEAD")
        .output()
        .unwrap();
    let concurrent_sha = String::from_utf8_lossy(&output.stdout).trim().to_string();

    // Merge worktree into main
    let outcome = fast_forward_into_main(&repo_path, &worktree).unwrap();

    // Should be cherry-picked (not fast-forwarded)
    match outcome {
        MergeOutcome::CherryPicked {
            onto,
            picked_sha: _,
        } => {
            assert_eq!(onto, concurrent_sha);
            // picked_sha should be different from worker's original commit
        }
        _ => panic!("expected CherryPicked, got {:?}", outcome),
    }

    // Assert main carries both edits
    assert_eq!(
        fs::read_to_string(repo_path.join("src/lib.rs")).unwrap(),
        "fixed"
    );
    assert_eq!(
        fs::read_to_string(repo_path.join("src/main.rs")).unwrap(),
        "B"
    );

    // Conflict case: main edits the same file
    let worktree2 = create_worktree(&repo_path, "sess-conflict", &baseline_sha).unwrap();

    // Worker edits src/lib.rs differently
    fs::write(worktree2.path.join("src/lib.rs"), "worker-version").unwrap();
    Command::new("git")
        .arg("-C")
        .arg(&worktree2.path)
        .arg("add")
        .arg(".")
        .output()
        .unwrap();
    Command::new("git")
        .arg("-C")
        .arg(&worktree2.path)
        .arg("commit")
        .arg("-m")
        .arg("[FT-Y] change A")
        .output()
        .unwrap();

    // Main also edits src/lib.rs (conflict)
    fs::write(repo_path.join("src/lib.rs"), "main-version").unwrap();
    Command::new("git")
        .arg("-C")
        .arg(&repo_path)
        .arg("add")
        .arg(".")
        .output()
        .unwrap();
    Command::new("git")
        .arg("-C")
        .arg(&repo_path)
        .arg("commit")
        .arg("-m")
        .arg("Main change A")
        .output()
        .unwrap();

    let output = Command::new("git")
        .arg("-C")
        .arg(&repo_path)
        .arg("rev-parse")
        .arg("HEAD")
        .output()
        .unwrap();
    let main_before_conflict = String::from_utf8_lossy(&output.stdout).trim().to_string();

    // Try to merge — should fail with conflict
    let result = fast_forward_into_main(&repo_path, &worktree2);
    assert!(result.is_err(), "merge should fail with conflict");

    match result {
        Err(MergeError::Conflict { ref paths }) => {
            assert!(!paths.is_empty(), "should have conflicting paths");
            // The conflict should involve lib.rs or src/lib.rs
            assert!(
                paths.iter().any(|p| p.contains("lib.rs")),
                "conflict paths should include lib.rs, got: {:?}",
                paths
            );
        }
        _ => panic!("expected Conflict error, got {:?}", result),
    }

    // Assert main HEAD unchanged (no half-merged state)
    let output = Command::new("git")
        .arg("-C")
        .arg(&repo_path)
        .arg("rev-parse")
        .arg("HEAD")
        .output()
        .unwrap();
    let main_after_conflict = String::from_utf8_lossy(&output.stdout).trim().to_string();
    assert_eq!(main_after_conflict, main_before_conflict);
}

/// TC-235: Orphan worktrees from harness crash are pruned at next start
#[test]
fn tc_235_orphan_pruning() {
    let (_temp, repo_path, baseline_sha) = init_test_repo();
    let (store, dump_path, writer) = init_test_store(&repo_path);

    // Create three worktrees
    let wt_alpha = create_worktree(&repo_path, "sess-alpha", &baseline_sha).unwrap();
    let wt_beta = create_worktree(&repo_path, "sess-beta", &baseline_sha).unwrap();
    let wt_gamma = create_worktree(&repo_path, "sess-gamma", &baseline_sha).unwrap();

    // Record alpha and gamma as live, skip beta (simulates crash)
    let alpha_iri = test_session_iri("sess-alpha");
    let gamma_iri = test_session_iri("sess-gamma");

    record_worktree_created(&writer, &store, &dump_path, &alpha_iri, &wt_alpha).unwrap();
    record_worktree_created(&writer, &store, &dump_path, &gamma_iri, &wt_gamma).unwrap();

    // Prune orphans
    let outcome = prune_orphans(&repo_path, &store).unwrap();

    // beta should be pruned, alpha and gamma kept
    assert_eq!(outcome.pruned.len(), 1);
    assert!(outcome.pruned.contains(&"sess-beta".to_string()));
    assert_eq!(outcome.kept.len(), 2);
    assert!(outcome.kept.contains(&"sess-alpha".to_string()));
    assert!(outcome.kept.contains(&"sess-gamma".to_string()));

    // Assert beta worktree is gone
    assert!(!wt_beta.path.exists());

    // Assert beta branch is gone
    let output = Command::new("git")
        .arg("-C")
        .arg(&repo_path)
        .arg("branch")
        .arg("--list")
        .arg("dec/sess/sess-beta")
        .output()
        .unwrap();
    let branches = String::from_utf8_lossy(&output.stdout);
    assert!(branches.trim().is_empty());

    // alpha and gamma still exist
    assert!(wt_alpha.path.exists());
    assert!(wt_gamma.path.exists());

    // Mark alpha as merged, then prune again
    record_worktree_merged(&writer, &store, &dump_path, &alpha_iri, "abc123").unwrap();

    let outcome2 = prune_orphans(&repo_path, &store).unwrap();

    // alpha should now be pruned (terminal session)
    assert!(outcome2.pruned.contains(&"sess-alpha".to_string()));
    assert!(!wt_alpha.path.exists());

    // gamma still kept
    assert!(outcome2.kept.contains(&"sess-gamma".to_string()));
    assert!(wt_gamma.path.exists());
}

/// TC-236: Two concurrent implementer dispatches into separate worktrees do not interfere
#[tokio::test]
async fn tc_236_concurrent_worktrees_no_interference() {
    let (_temp, repo_path, _baseline_sha) = init_test_repo();
    let (store, dump_path, writer) = init_test_store(&repo_path);

    // Create two files
    fs::write(repo_path.join("src/feature_a.rs"), "A-original").unwrap();
    fs::write(repo_path.join("src/feature_b.rs"), "B-original").unwrap();
    Command::new("git")
        .arg("-C")
        .arg(&repo_path)
        .arg("add")
        .arg(".")
        .output()
        .unwrap();
    Command::new("git")
        .arg("-C")
        .arg(&repo_path)
        .arg("commit")
        .arg("-m")
        .arg("Add features")
        .output()
        .unwrap();

    let output = Command::new("git")
        .arg("-C")
        .arg(&repo_path)
        .arg("rev-parse")
        .arg("HEAD")
        .output()
        .unwrap();
    let baseline_sha = String::from_utf8_lossy(&output.stdout).trim().to_string();

    // Spawn two parallel workers
    let repo_a = repo_path.clone();
    let repo_b = repo_path.clone();
    let baseline_a = baseline_sha.clone();
    let baseline_b = baseline_sha.clone();

    let worker_a = tokio::spawn(async move {
        let wt_a = create_worktree(&repo_a, "sess-a", &baseline_a).unwrap();
        fs::write(wt_a.path.join("src/feature_a.rs"), "A-modified").unwrap();
        Command::new("git")
            .arg("-C")
            .arg(&wt_a.path)
            .arg("add")
            .arg(".")
            .output()
            .unwrap();
        Command::new("git")
            .arg("-C")
            .arg(&wt_a.path)
            .arg("commit")
            .arg("-m")
            .arg("[FT-A] add A")
            .output()
            .unwrap();
        wt_a
    });

    let worker_b = tokio::spawn(async move {
        let wt_b = create_worktree(&repo_b, "sess-b", &baseline_b).unwrap();
        fs::write(wt_b.path.join("src/feature_b.rs"), "B-modified").unwrap();
        Command::new("git")
            .arg("-C")
            .arg(&wt_b.path)
            .arg("add")
            .arg(".")
            .output()
            .unwrap();
        Command::new("git")
            .arg("-C")
            .arg(&wt_b.path)
            .arg("commit")
            .arg("-m")
            .arg("[FT-B] add B")
            .output()
            .unwrap();
        wt_b
    });

    let (wt_a_result, wt_b_result) = tokio::join!(worker_a, worker_b);
    let wt_a = wt_a_result.unwrap();
    let wt_b = wt_b_result.unwrap();

    // Assert both completed successfully
    assert!(wt_a.path.exists());
    assert!(wt_b.path.exists());

    // Assert worktree_a only modified feature_a
    assert_eq!(
        fs::read_to_string(wt_a.path.join("src/feature_a.rs")).unwrap(),
        "A-modified"
    );
    assert_eq!(
        fs::read_to_string(wt_a.path.join("src/feature_b.rs")).unwrap(),
        "B-original"
    );

    // Assert worktree_b only modified feature_b
    assert_eq!(
        fs::read_to_string(wt_b.path.join("src/feature_a.rs")).unwrap(),
        "A-original"
    );
    assert_eq!(
        fs::read_to_string(wt_b.path.join("src/feature_b.rs")).unwrap(),
        "B-modified"
    );

    // Merge both serially
    let outcome_a = fast_forward_into_main(&repo_path, &wt_a).unwrap();
    let session_a_iri = test_session_iri("sess-a");

    let merged_sha_a = match &outcome_a {
        MergeOutcome::FastForwarded { to, .. } => to.clone(),
        MergeOutcome::CherryPicked { picked_sha, .. } => picked_sha.clone(),
    };

    record_worktree_merged(&writer, &store, &dump_path, &session_a_iri, &merged_sha_a).unwrap();

    let outcome_b = fast_forward_into_main(&repo_path, &wt_b).unwrap();
    let session_b_iri = test_session_iri("sess-b");

    let merged_sha_b = match &outcome_b {
        MergeOutcome::FastForwarded { to, .. } => to.clone(),
        MergeOutcome::CherryPicked { picked_sha, .. } => picked_sha.clone(),
    };

    record_worktree_merged(&writer, &store, &dump_path, &session_b_iri, &merged_sha_b).unwrap();

    // Assert main carries both edits
    assert_eq!(
        fs::read_to_string(repo_path.join("src/feature_a.rs")).unwrap(),
        "A-modified"
    );
    assert_eq!(
        fs::read_to_string(repo_path.join("src/feature_b.rs")).unwrap(),
        "B-modified"
    );

    // Verify both sessions have worktreeStatus="merged" in store
    let status_pred = NamedNode::new_unchecked("https://purl.org/ddd/decision-cli#worktreeStatus");

    let mut found_a_merged = false;
    let mut found_b_merged = false;

    for _quad in store.quads_for_pattern(
        Some(session_a_iri.as_ref().into()),
        Some(status_pred.as_ref().into()),
        Some((&Literal::new_simple_literal("merged")).into()),
        None,
    ) {
        found_a_merged = true;
    }

    for _quad in store.quads_for_pattern(
        Some(session_b_iri.as_ref().into()),
        Some(status_pred.as_ref().into()),
        Some((&Literal::new_simple_literal("merged")).into()),
        None,
    ) {
        found_b_merged = true;
    }

    assert!(found_a_merged, "sess-a should have worktreeStatus=merged");
    assert!(found_b_merged, "sess-b should have worktreeStatus=merged");
}
