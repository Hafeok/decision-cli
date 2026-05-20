//! TC-018 — `dec implement` commits the working tree and flips feature status.
//!
//! Validates: FT-017 · ADR-009, ADR-010, ADR-011.
//! Spec: `.product/tests/TC-018-dec-implement-commits-the-working-tree-and-flips-f.md`
//!
//! The acceptance criteria the test asserts (taken verbatim from the TC):
//!
//!   1. `git status --porcelain` returns empty after the run.
//!   2. `git log -1 --format=%s` matches `^\[FT-XXX\] `.
//!   3. `git log -1 --format=%B` contains `Session:`, `Dispatch:`,
//!      `CodeChange:`, and `Bundle: sha256:`.
//!   4. The Session IRI cited in the commit body resolves in
//!      `.dec/store/orchestration.nq` with `dec:status "complete"`.
//!   5. `product feature show FT-XXX --format json` reports
//!      `"status": "complete"`.
//!   6. A deliberately failing pre-commit hook surfaces a
//!      `FinalizeError::CommitFailed` (hooks are NOT bypassed).
//!
//! Fixture: a throwaway git repo in `tempdir()` with `.dec/` and
//! `.product/` seeded; the worker is the deterministic stub
//! (`CODE_WRITER_STUB=1`).

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use decision_cli::implement::{run as implement_run, ImplementArgs};
use decision_cli::init::{run as init_run, DefinitionSource};
use oxigraph::io::RdfFormat;
use oxigraph::sparql::QueryResults;
use oxigraph::store::Store;

const FEATURE_ID: &str = "FT-099";

#[test]
fn dec_implement_commits_the_working_tree_and_flips_feature_status() {
    if which("git").is_none() {
        eprintln!("TC-018: git not on PATH — skipping");
        return;
    }
    if which("product").is_none() {
        eprintln!("TC-018: product CLI not on PATH — skipping");
        return;
    }

    env::set_var("CODE_WRITER_STUB", "1");
    env::remove_var("CODE_WRITER_CMD");

    happy_path();
    failing_pre_commit_hook_is_honoured();
}

/// Acceptance criteria #1–#5: clean working tree, commit subject + body,
/// Session at `complete` in the orchestration store, feature at `complete`
/// in the product graph.
fn happy_path() {
    let workdir = fresh_workdir("tc-018-happy");
    init_git_repo(&workdir);
    init_run(
        &workdir,
        DefinitionSource::Template("engineering-development".into()),
    )
    .expect("dec init");
    seed_product_fixture(&workdir, FEATURE_ID);

    let mut args = ImplementArgs::new(FEATURE_ID);
    args.product_root = Some(workdir.clone());
    let outcome = implement_run(&workdir, &args).expect("dec implement");

    // #1 working tree clean.
    let porcelain = git_porcelain(&workdir);
    assert!(
        porcelain.is_empty(),
        "TC-018 #1: working tree dirty after run: {porcelain:?}"
    );

    // #2 commit subject starts with `[FT-099] `.
    let subject = git_log_format(&workdir, "%s");
    assert!(
        subject.starts_with(&format!("[{FEATURE_ID}] ")),
        "TC-018 #2: subject does not start with `[{FEATURE_ID}] `: {subject:?}"
    );

    // #3 body contains the four PROV-O reference lines.
    //
    // The TC text quotes `Bundle: sha256:` with a single space, but
    // FT-017's body template uses aligned columns
    // (`Bundle:      sha256:`). Honour the alignment by accepting any
    // run of whitespace between the label and the `sha256:` prefix.
    let body = git_log_format(&workdir, "%B");
    for needle in ["Session:", "Dispatch:", "CodeChange:"] {
        assert!(
            body.contains(needle),
            "TC-018 #3: body missing {needle:?}: {body}"
        );
    }
    assert!(
        body.lines()
            .any(|l| l.trim_start().starts_with("Bundle:") && l.contains("sha256:")),
        "TC-018 #3: body missing a `Bundle: …sha256:…` line: {body}"
    );
    // Body must also contain the actual minted Session IRI.
    assert!(
        body.contains(&outcome.session_iri),
        "TC-018 #3: body missing Session IRI {}: {body}",
        outcome.session_iri
    );

    // #4 Session is `complete` in the orchestration store.
    let store = load_orchestration_store(&workdir);
    let q = format!(
        r#"PREFIX dec: <https://decision-cli.dev/ns#>
ASK {{ GRAPH ?g {{ <{iri}> dec:status "complete" }} }}"#,
        iri = outcome.session_iri,
    );
    match store.query(&q).expect("ask runs") {
        QueryResults::Boolean(true) => {}
        QueryResults::Boolean(false) => panic!(
            "TC-018 #4: session {} not at status=complete in orchestration.nq",
            outcome.session_iri
        ),
        _ => panic!("TC-018 #4: ASK query returned non-boolean result"),
    }

    // #5 product feature show reports `"status": "complete"`.
    let status = product_feature_status_json(&workdir, FEATURE_ID);
    assert_eq!(
        status, "complete",
        "TC-018 #5: product feature show reports status={status:?}"
    );

    // Bonus sanity: the harness reported the commit SHA and the
    // transition flag, surfaced into the ImplementOutcome.
    let fin = outcome.finalize.as_ref().expect("FinalizeOutcome present");
    assert!(
        fin.commit_sha.is_some(),
        "TC-018: ImplementOutcome.finalize.commit_sha must be set"
    );
    assert!(
        fin.status_transitioned,
        "TC-018: ImplementOutcome.finalize.status_transitioned must be true"
    );
}

/// Acceptance criterion #6: a failing pre-commit hook causes the run to
/// surface a `FinalizeError::CommitFailed`.
fn failing_pre_commit_hook_is_honoured() {
    let workdir = fresh_workdir("tc-018-hook");
    init_git_repo(&workdir);
    init_run(
        &workdir,
        DefinitionSource::Template("engineering-development".into()),
    )
    .expect("dec init");
    seed_product_fixture(&workdir, FEATURE_ID);
    install_failing_pre_commit(&workdir);

    let mut args = ImplementArgs::new(FEATURE_ID);
    args.product_root = Some(workdir.clone());
    let err = implement_run(&workdir, &args)
        .expect_err("TC-018 #6: failing pre-commit hook must abort the run");

    // The chain is wrapped by anyhow `.context("finalising dec implement run (FT-017)")`
    // around `FinalizeError::CommitFailed { detail }`. Display message
    // shows the root cause "git commit failed: …" somewhere in the chain.
    let chain = format!("{err:#}");
    assert!(
        chain.contains("git commit failed"),
        "TC-018 #6: error chain does not surface FinalizeError::CommitFailed: {chain}"
    );
}

// ---------------------------------------------------------------------
// Fixture helpers.
// ---------------------------------------------------------------------

fn fresh_workdir(tag: &str) -> PathBuf {
    let mut base = env::temp_dir();
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    base.push(format!(
        "decision-cli-{tag}-{}-{}",
        std::process::id(),
        nanos
    ));
    fs::create_dir_all(&base).expect("create temp workdir");
    base
}

fn init_git_repo(workdir: &Path) {
    run_must_succeed(Command::new("git").arg("-C").arg(workdir).args(["init", "-q"]));
    // Local user.* config so commit doesn't depend on the host's git
    // identity (tests must be hermetic).
    run_must_succeed(
        Command::new("git")
            .arg("-C")
            .arg(workdir)
            .args(["config", "user.email", "tc-018@decision-cli.test"]),
    );
    run_must_succeed(
        Command::new("git")
            .arg("-C")
            .arg(workdir)
            .args(["config", "user.name", "tc-018"]),
    );
    // Ensure HEAD exists so the implementer's commit lands on top of
    // something. The TC spec calls this out explicitly.
    run_must_succeed(
        Command::new("git")
            .arg("-C")
            .arg(workdir)
            .args(["commit", "--allow-empty", "-q", "-m", "initial"]),
    );
}

fn seed_product_fixture(workdir: &Path, feature_id: &str) {
    let product_dir = workdir.join(".product");
    let features_dir = product_dir.join("features");
    fs::create_dir_all(&features_dir).expect("create .product/features");
    fs::create_dir_all(product_dir.join("adrs")).expect("create .product/adrs");
    fs::create_dir_all(product_dir.join("tests")).expect("create .product/tests");
    fs::create_dir_all(product_dir.join("graph")).expect("create .product/graph");

    // Minimal config.toml — product-cli needs it to resolve the graph.
    fs::write(
        product_dir.join("config.toml"),
        r#"name = "tc-018-fixture"
schema-version = "1"

[product]
responsibility = "throwaway fixture for TC-018"

[paths]
features = ".product/features"
adrs = ".product/adrs"
tests = ".product/tests"
graph = ".product/graph"
requests = ".product/requests.jsonl"

[prefixes]
feature = "FT"
adr = "ADR"
test = "TC"

[phases]
1 = "Phase 1"
"#,
    )
    .expect("write .product/config.toml");

    // Minimal feature_spec at status `in-progress`.
    let body = format!(
        "---\n\
id: {feature_id}\n\
title: TC-018 fixture feature\n\
phase: 1\n\
status: in-progress\n\
depends-on: []\n\
adrs: []\n\
tests: []\n\
domains: []\n\
domains-acknowledged: {{}}\n\
---\n\
\n\
## Description\n\
\n\
TC-018 fixture — written by the integration test, not by a human.\n",
    );
    let path = features_dir.join(format!("{feature_id}-tc-018-fixture-feature.md"));
    fs::write(&path, body).expect("write feature_spec");
}

fn install_failing_pre_commit(workdir: &Path) {
    let hooks = workdir.join(".git").join("hooks");
    fs::create_dir_all(&hooks).expect("create .git/hooks");
    let hook = hooks.join("pre-commit");
    fs::write(
        &hook,
        "#!/bin/sh\necho 'tc-018: pre-commit hook deliberately failing' >&2\nexit 17\n",
    )
    .expect("write pre-commit hook");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&hook).expect("hook metadata").permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&hook, perms).expect("chmod hook");
    }
}

fn run_must_succeed(cmd: &mut Command) {
    let out = cmd.output().expect("spawn");
    if !out.status.success() {
        panic!(
            "command {cmd:?} failed: {}\n{}",
            out.status,
            String::from_utf8_lossy(&out.stderr)
        );
    }
}

fn git_porcelain(workdir: &Path) -> String {
    let out = Command::new("git")
        .arg("-C")
        .arg(workdir)
        .args(["status", "--porcelain"])
        .output()
        .expect("git status");
    assert!(out.status.success(), "git status non-zero");
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn git_log_format(workdir: &Path, fmt: &str) -> String {
    let out = Command::new("git")
        .arg("-C")
        .arg(workdir)
        .args(["log", "-1", &format!("--format={fmt}")])
        .output()
        .expect("git log");
    assert!(out.status.success(), "git log non-zero");
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn load_orchestration_store(workdir: &Path) -> Store {
    let dump = workdir.join(".dec").join("store").join("orchestration.nq");
    let bytes = fs::read(&dump).expect("read orchestration.nq dump");
    let store = Store::new().expect("in-memory store");
    store
        .load_from_reader(RdfFormat::NQuads, bytes.as_slice())
        .expect("load orchestration dump");
    store
}

fn product_feature_status_json(workdir: &Path, feature_id: &str) -> String {
    let out = Command::new("product")
        .arg("feature")
        .arg("show")
        .arg(feature_id)
        .arg("--format")
        .arg("json")
        .arg("--root")
        .arg(workdir)
        .output()
        .expect("spawn product feature show");
    assert!(
        out.status.success(),
        "product feature show non-zero: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let raw = String::from_utf8_lossy(&out.stdout).into_owned();
    extract_status_field(&raw).unwrap_or_else(|| {
        panic!(
            "TC-018 #5: could not find \"status\" in product feature show JSON: {raw}"
        )
    })
}

fn extract_status_field(raw: &str) -> Option<String> {
    let key = "\"status\":";
    let idx = raw.find(key)?;
    let rest = &raw[idx + key.len()..].trim_start();
    let rest = rest.strip_prefix('"')?;
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

fn which(bin: &str) -> Option<PathBuf> {
    let path = env::var_os("PATH")?;
    for dir in env::split_paths(&path) {
        let candidate = dir.join(bin);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}
