//! FT-025 — `dec session show <iri>` displays paired session + verdict.
//!
//! Validates TC-031: when a Session belongs to a `dec:DispatchGroup`
//! (ADR-017), `dec session show` appends a "Paired:" block carrying
//! the group's status, the paired session IRI, and — when a verifier
//! `VerificationVerdict` (ADR-018) exists — the verdict value, its
//! rationale, the `dec:violates` references, and the
//! `dec:amendmentGuidance` text.
//!
//! We exercise the `session_show` public function directly against a
//! temp-dir orchestration store seeded with synthetic N-Quads (the same
//! RDF shape `dec implement` + the verifier worker would write).

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use decision_cli::implement::session_show;

const SESSION_NQ: &str = include_str!("data/ft_025_paired_session.nq");

static COUNTER: AtomicU64 = AtomicU64::new(0);

struct TempWorkdir {
    path: PathBuf,
}

impl TempWorkdir {
    fn new(nq: &str) -> Self {
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let pid = std::process::id();
        let path = std::env::temp_dir().join(format!("dec-ft025-{pid}-{n}"));
        let store_dir = path.join(".dec").join("store");
        fs::create_dir_all(&store_dir).expect("mkdir .dec/store");
        fs::write(store_dir.join("orchestration.nq"), nq).expect("write nq dump");
        Self { path }
    }
    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempWorkdir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn render(iri: &str) -> String {
    let dir = TempWorkdir::new(SESSION_NQ);
    session_show(dir.path(), iri).expect("session_show ok")
}

#[test]
fn action_session_renders_paired_block_with_verdict() {
    let out = render("urn:dec:test:ft-025:session/action");
    // Slice-1 fields stay present.
    assert!(out.contains("Session: urn:dec:test:ft-025:session/action"));
    assert!(out.contains("Feature:        FT-025"));
    // FT-025 paired block.
    assert!(out.contains("Paired:"), "missing Paired: header\n{out}");
    assert!(
        out.contains("Dispatch group: urn:dec:test:ft-025:group"),
        "missing dispatch group iri\n{out}"
    );
    assert!(
        out.contains("Status:         complete"),
        "missing paired status\n{out}"
    );
    assert!(
        out.contains("Interpretation: urn:dec:test:ft-025:session/interpretation"),
        "missing interpretation session iri\n{out}"
    );
    // Verdict block.
    assert!(
        out.contains("Verdict:        approved"),
        "missing verdict value\n{out}"
    );
    assert!(
        out.contains("Verdict IRI:"),
        "missing verdict IRI line\n{out}"
    );
    assert!(out.contains("Rationale:"), "missing rationale block\n{out}");
    assert!(out.contains("satisfies"), "missing rationale prose\n{out}");
}

#[test]
fn interpretation_session_renders_paired_action_session() {
    let out = render("urn:dec:test:ft-025:session/interpretation");
    assert!(out.contains("Paired:"), "missing Paired: header\n{out}");
    assert!(
        out.contains("Action: urn:dec:test:ft-025:session/action"),
        "interpretation-side paired view should name the action session\n{out}"
    );
    assert!(out.contains("Verdict:        approved"));
}

#[test]
fn standalone_session_omits_paired_block() {
    // The standalone session has its own bundle/model/in-stream wires
    // but is not part of any DispatchGroup.
    let dir = TempWorkdir::new(SESSION_NQ);
    let out =
        session_show(dir.path(), "urn:dec:test:ft-025:session/standalone").expect("standalone ok");
    assert!(
        !out.contains("Paired:"),
        "standalone session should NOT carry a Paired: block\n{out}"
    );
    assert!(!out.contains("Verdict:"));
}

#[test]
fn amendment_required_verdict_renders_guidance() {
    let out = render("urn:dec:test:ft-025:session/action-amend");
    assert!(
        out.contains("Verdict:        amendment-required"),
        "amendment verdict missing\n{out}"
    );
    assert!(
        out.contains("Amendment guidance:"),
        "amendment guidance block missing\n{out}"
    );
    assert!(out.contains("Violates:"), "violates block missing\n{out}");
    assert!(
        out.contains("urn:dec:test:ft-025:tc/example"),
        "violates IRI missing\n{out}"
    );
}
