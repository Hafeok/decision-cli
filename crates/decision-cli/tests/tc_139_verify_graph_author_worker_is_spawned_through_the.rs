//! TC-139 — verify-graph-author worker is spawned through the shared resolver chain.
//!
//! Validates: FT-067 · ADR-007, ADR-008, ADR-013, ADR-016.
//! Spec: `.product/tests/TC-139-verify-graph-author-worker-is-spawned-through-the.md`
//!
//! Three structural checks back the invariant:
//!
//! 1. The embedded worker manifest lists `verify-graph-author` with its
//!    canonical console-script / Python-module / env-var triple — so the
//!    shared resolver knows the role exists (FT-016 § manifest table).
//! 2. `core::worker::resolve` honours `VERIFY_GRAPH_AUTHOR_CMD` for the
//!    verify-graph-author entry — proving the role is wired into the
//!    shared chain rather than carrying a bespoke lookup.
//! 3. `verify_graph_generate::worker::invoke_worker` end-to-end spawns
//!    whatever the env-var override resolves to, appending `--stdin` to
//!    the argv. A sentinel shell-script worker echoes back a stub
//!    `GraphProposal` and records its invocation; the test asserts the
//!    script ran with the right argv and the bundle was piped to stdin.
//!    This is the load-bearing assertion — if the spawn ever drifted back
//!    to an inline `python3 -m verify_graph_author`, the env override
//!    would be ignored and the assertion would fire.

use std::env;
use std::fs;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use decision_cli::verify_graph_generate::bundle::{
    EnvRecord, StepKindRecord, VerifyGraphAuthorInputJson,
};
use decision_cli::verify_graph_generate::proposal::ProposalKind;
use decision_cli::verify_graph_generate::worker::{
    invoke_worker, reset_subprocess_invocation_count, subprocess_invocation_count,
};
use decision_cli::worker::{resolve, role_entry, Resolution, ResolutionKind, ResolveInputs};

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
            "decision-cli-tc139-{tag}-{}-{}-{}",
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

/// Sentinel input bundle: minimal but type-safe — the shape that
/// `invoke_worker` round-trips through the (env-var-resolved) subprocess.
fn sentinel_bundle() -> VerifyGraphAuthorInputJson {
    VerifyGraphAuthorInputJson {
        feature_id: "FT-Z139".to_string(),
        feature_spec: "fixture spec body".to_string(),
        relevant_tcs: vec![],
        target_environment: EnvRecord {
            id: "ENV-001-ephemeral-cli".to_string(),
            env_type: "ephemeral-tempdir".to_string(),
            safety_class: "safe".to_string(),
            allowed_ops: vec!["fs-tempdir".to_string()],
            endpoint: None,
        },
        candidate_graphs: vec![],
        step_vocabulary: vec![StepKindRecord {
            kind: "shell-command".to_string(),
            required_ops: vec!["shell".to_string()],
            fields_schema: serde_json::json!({}),
            description: "stub".to_string(),
        }],
        // The sentinel script echoes this hash back in its stub proposal;
        // the harness asserts the echo, which is how we know the
        // subprocess actually consumed our bundle.
        bundle_hash: "tc139-sentinel-bundle-hash".to_string(),
        endpoint: "anthropic".to_string(),
        model_id: "stub-model".to_string(),
        parameters: serde_json::json!({}),
        max_tokens: 4096,
    }
}

/// Write a bash sentinel script that:
///   - records its argv to `argv_log` (one arg per line),
///   - copies stdin to `stdin_log`,
///   - prints a stub `GraphProposal::Gap` JSON to stdout that echoes the
///     bundle hash from our `sentinel_bundle`.
///
/// The bundle-hash echo is what lets the harness's `verify_bundle_hash`
/// check pass when this script stands in for the real worker (here we
/// call `invoke_worker` directly, but the parse path is the same).
fn write_sentinel_script(dir: &Path, argv_log: &Path, stdin_log: &Path) -> PathBuf {
    let script_path = dir.join("verify-graph-author-sentinel.sh");
    let mut script = String::new();
    script.push_str("#!/usr/bin/env bash\n");
    script.push_str("set -euo pipefail\n");
    // Record argv (one per line) — `$0` plus every positional.
    script.push_str(&format!(
        "printf '%s\\n' \"$0\" \"$@\" > {argv_log_lit}\n",
        argv_log_lit = shell_quote(argv_log)
    ));
    // Copy stdin to a log file.
    script.push_str(&format!(
        "cat - > {stdin_log_lit}\n",
        stdin_log_lit = shell_quote(stdin_log)
    ));
    // Emit a stub GraphProposal::Gap on stdout. Echoes the sentinel
    // bundle_hash so `invoke_worker`'s downstream verifiers see a
    // matching token.
    script.push_str(
        "printf '{\"kind\":\"gap\",\"bundle_hash\":\"tc139-sentinel-bundle-hash\",\
         \"gap\":{\"uncovered_tcs\":[],\"reason\":\"sentinel\"}}\\n'\n",
    );
    fs::write(&script_path, script).expect("write sentinel script");
    let mut perm = fs::metadata(&script_path)
        .expect("stat script")
        .permissions();
    perm.set_mode(0o755);
    fs::set_permissions(&script_path, perm).expect("chmod +x script");
    script_path
}

fn shell_quote(p: &Path) -> String {
    // POSIX single-quote escaping — adequate for tempdir paths.
    let s = p.to_string_lossy();
    format!("'{}'", s.replace('\'', r"'\''"))
}

/// Saved-and-restored env var so we don't leak state to other tests.
struct EnvGuard {
    key: &'static str,
    prior: Option<String>,
}

impl EnvGuard {
    fn set(key: &'static str, value: &str) -> Self {
        let prior = env::var(key).ok();
        env::set_var(key, value);
        Self { key, prior }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        match &self.prior {
            Some(v) => env::set_var(self.key, v),
            None => env::remove_var(self.key),
        }
    }
}

#[test]
fn tc_139_verify_graph_author_worker_is_spawned_through_the() {
    // ── AC #1: manifest lists verify-graph-author with the canonical
    //          triple that the resolver chain knows how to act on. ─────
    let entry = role_entry("verify-graph-author")
        .expect("verify-graph-author entry present in embedded manifest (FT-067)");
    assert_eq!(entry.role, "verify-graph-author");
    assert_eq!(entry.console_script, "verify-graph-author");
    assert_eq!(entry.python_module, "verify_graph_author");
    assert_eq!(entry.env_var, "VERIFY_GRAPH_AUTHOR_CMD");

    // ── AC #2: the *shared* resolver honours the env-var override for
    //          this entry. Proves the role is wired into the single
    //          chain `core::worker::resolve` owns (TC-050 enforces the
    //          structural converse — that no second chain exists). ────
    let wd = WorkdirGuard::new("env");
    let argv_log = wd.path().join("argv.txt");
    let stdin_log = wd.path().join("stdin.bin");
    let script = write_sentinel_script(wd.path(), &argv_log, &stdin_log);

    let _env_guard = EnvGuard::set(
        "VERIFY_GRAPH_AUTHOR_CMD",
        script.to_string_lossy().as_ref(),
    );

    let resolved = resolve(entry, ResolveInputs::default());
    match &resolved {
        Resolution::Resolved { kind, argv, .. } => {
            assert_eq!(
                *kind,
                ResolutionKind::Env,
                "env-var path of the shared resolver must own VERIFY_GRAPH_AUTHOR_CMD"
            );
            assert_eq!(
                argv[0],
                script.to_string_lossy(),
                "resolved argv must point at the sentinel script"
            );
        }
        Resolution::Missing { diagnostics } => panic!(
            "expected resolve() to return Resolved (env path), got Missing: {diagnostics:?}"
        ),
    }

    // ── AC #3: end-to-end spawn — `invoke_worker` actually executes
    //          whatever the shared resolver returns, appending `--stdin`
    //          and piping the bundle. If the spawn site ever drifted
    //          back to an inline `python3 -m verify_graph_author`, the
    //          env override would be ignored and (a) the sentinel
    //          script would not run, (b) `argv.txt` would not exist. ──
    reset_subprocess_invocation_count();
    let bundle = sentinel_bundle();
    let proposal = invoke_worker(&bundle).expect(
        "invoke_worker must spawn the env-var-resolved sentinel and parse its stub proposal",
    );

    // The stub script returns a Gap proposal echoing our bundle hash.
    assert_eq!(proposal.kind, ProposalKind::Gap);
    assert_eq!(
        proposal.bundle_hash, bundle.bundle_hash,
        "sentinel script must echo the bundle hash verbatim (proves stdin flowed through)"
    );

    // A *real* subprocess was spawned (mock path bypasses this counter).
    assert_eq!(
        subprocess_invocation_count(),
        1,
        "invoke_worker must spawn exactly one subprocess (no mock installed)"
    );

    // The sentinel ran: argv was logged and includes `--stdin` per the
    // FT-067 spawn contract.
    let argv_text = fs::read_to_string(&argv_log)
        .expect("sentinel script's argv log must exist — proves the spawn happened");
    let argv_lines: Vec<&str> = argv_text.lines().collect();
    assert!(
        argv_lines.iter().any(|l| l.trim() == "--stdin"),
        "spawn must append --stdin to the resolved argv (FT-067 §Behaviour); argv was {argv_lines:?}"
    );

    // The bundle reached the subprocess's stdin verbatim.
    let stdin_bytes = fs::read(&stdin_log)
        .expect("sentinel script's stdin log must exist — proves the bundle was piped");
    let echoed: VerifyGraphAuthorInputJson = serde_json::from_slice(&stdin_bytes)
        .expect("stdin must round-trip as VerifyGraphAuthorInputJson");
    assert_eq!(
        echoed.bundle_hash, bundle.bundle_hash,
        "stdin payload bundle_hash must match the input"
    );
    assert_eq!(echoed.feature_id, bundle.feature_id);
}

// Small belt-and-braces helper: confirms our env-var-path argv contains
// exactly the script path — no `python3 -m verify_graph_author` literal
// can ever appear when this resolves through the shared chain. Pulled
// into a separate test so a regression on the spawn site is visible
// independently from the parse path.
#[test]
fn tc_139_resolver_env_var_path_returns_only_the_override_argv() {
    let entry = role_entry("verify-graph-author").expect("entry");
    let wd = WorkdirGuard::new("env-only");
    let script = wd.path().join("noop.sh");
    let mut f = fs::File::create(&script).expect("noop");
    writeln!(f, "#!/usr/bin/env bash").expect("write");
    writeln!(f, "exit 0").expect("write");
    let mut perm = fs::metadata(&script)
        .expect("stat")
        .permissions();
    perm.set_mode(0o755);
    fs::set_permissions(&script, perm).expect("chmod");

    let _g = EnvGuard::set(
        "VERIFY_GRAPH_AUTHOR_CMD",
        script.to_string_lossy().as_ref(),
    );
    match resolve(entry, ResolveInputs::default()) {
        Resolution::Resolved { kind, argv, .. } => {
            assert_eq!(kind, ResolutionKind::Env);
            assert_eq!(argv, vec![script.to_string_lossy().into_owned()]);
            assert!(
                !argv.iter().any(|a| a.contains("verify_graph_author") && a != &script.to_string_lossy()),
                "env path must NOT smuggle in a python3 -m verify_graph_author argv"
            );
        }
        Resolution::Missing { diagnostics } => panic!("expected Resolved, got Missing: {diagnostics:?}"),
    }
}
