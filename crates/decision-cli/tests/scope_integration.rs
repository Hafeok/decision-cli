//! Scope integration tests (relocated from core/scope/tests.rs by FT-168).
//!
//! These exercise ActiveScope through the real `dec init` path, so they
//! live as a decision-cli integration test rather than inside dec-graph
//! (which must not depend on the binary crate's init machinery).

use std::path::{Path, PathBuf};

use decision_cli::core::bundled;
use decision_cli::features::init::{self, DefinitionSource};

use decision_cli::core::scope::{ActiveScope, ScopeError};

fn init_dev_stream(workdir: &Path) {
    init::run(
        workdir,
        DefinitionSource::Template("engineering-development".to_string()),
    )
    .expect("init succeeds");
}

#[test]
fn load_after_init_caches_authorized_goals() {
    let tmp = tempdir();
    init_dev_stream(tmp.path());
    let scope = ActiveScope::load(tmp.path()).expect("scope loads");
    assert!(scope.authorized_goals.contains(&"ship".to_string()));
    assert!(scope.authorized_goals.contains(&"land".to_string()));
    assert_eq!(
        scope.value_action_iri,
        bundled::SHIPPED_FEATURE_IRI.to_string()
    );
}

#[test]
fn authorized_goal_passes() {
    let tmp = tempdir();
    init_dev_stream(tmp.path());
    let scope = ActiveScope::load(tmp.path()).expect("scope loads");
    assert!(scope.validate_goal("ship").is_ok());
    assert!(scope.validate_goal("land").is_ok());
}

#[test]
fn unauthorized_goal_is_refused_with_full_diagnostic() {
    let tmp = tempdir();
    init_dev_stream(tmp.path());
    let scope = ActiveScope::load(tmp.path()).expect("scope loads");
    let err = scope.validate_goal("prioritize").expect_err("refused");
    let msg = err.to_string();
    assert!(msg.contains("prioritize"), "names goal: {msg}");
    assert!(msg.contains("ship"), "names authorized goal ship: {msg}");
    assert!(msg.contains("land"), "names authorized goal land: {msg}");
    assert!(
        msg.contains("va:shipped-feature"),
        "names ValueAction in prefixed form: {msg}"
    );
    assert!(
        msg.contains(bundled::SHIPPED_FEATURE_IRI),
        "names ValueAction full IRI: {msg}"
    );
    assert!(
        msg.contains("This stream pursues"),
        "matches §3.4 phrasing: {msg}"
    );
}

#[test]
fn uninitialized_workdir_errors_clearly() {
    let tmp = tempdir();
    let err = ActiveScope::load(tmp.path()).expect_err("uninitialised");
    assert!(matches!(err, ScopeError::Uninitialized { .. }));
}

fn tempdir() -> TempDir {
    let mut p = std::env::temp_dir();
    let pid = std::process::id();
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    p.push(format!("dec-scope-test-{pid}-{nonce}"));
    std::fs::create_dir_all(&p).expect("create tempdir");
    TempDir { path: p }
}

struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}
