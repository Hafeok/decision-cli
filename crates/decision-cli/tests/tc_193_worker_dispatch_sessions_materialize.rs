//! TC-193 — `core::dispatch_session::materialize` emits the typing +
//! provenance quads that make a worker-dispatch IRI visible in
//! `dec session list`. Idempotent re-call is a no-op.
//!
//! Full integration of the verify-graph-generate / implement dispatch
//! paths is exercised by FT-107 / FT-108 tests; this TC focuses on
//! the helper FT-109 added to bridge the gap.
//!
//! Validates: FT-109 · ADR-004.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use decision_cli::core::dispatch_session::{materialize, DispatchStatus};
use decision_cli::init::{run as init_run, DefinitionSource};
use oxigraph::model::NamedNode;

static COUNTER: AtomicU64 = AtomicU64::new(0);

const STREAM_TTL: &str =
    include_str!("../src/core/bundled/assets/streams/engineering-development.ttl");

struct WorkdirGuard(PathBuf);

impl WorkdirGuard {
    fn new(tag: &str) -> Self {
        let mut base = env::temp_dir();
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        let counter = COUNTER.fetch_add(1, Ordering::Relaxed);
        base.push(format!(
            "decision-cli-tc193-{tag}-{}-{}-{}",
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

#[test]
fn tc_193_worker_dispatch_sessions_materialize() {
    let wd = WorkdirGuard::new("materialize");
    let seed = write_seed_definition(wd.path());
    init_run(wd.path(), DefinitionSource::File(seed)).expect("dec init");

    // ----- Scenario A: verify-graph-author dispatch IRI -----
    let vga_iri = NamedNode::new_unchecked(
        "https://decision-cli.dev/ns/activity/verify-graph-generate/VG-T193",
    );
    let written = materialize(
        wd.path(),
        &vga_iri,
        "verify-graph-author",
        "FT-T193a",
        "2026-05-27T10:00:00Z",
        "2026-05-27T10:00:30Z",
        DispatchStatus::Completed,
    )
    .expect("vga materialize");
    assert!(written, "first call must commit new quads");

    // Idempotent re-call: a second materialize is a no-op.
    let again = materialize(
        wd.path(),
        &vga_iri,
        "verify-graph-author",
        "FT-T193a",
        "2026-05-27T10:00:00Z",
        "2026-05-27T10:00:30Z",
        DispatchStatus::Completed,
    )
    .expect("vga re-materialize");
    assert!(!again, "second call against already-typed IRI is a no-op");

    let dump_body = fs::read_to_string(wd.path().join(".dec/store/orchestration.nq"))
        .expect("read dump");
    // dec:Session typing quad lands.
    let session_class_line = format!(
        "<{}> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <https://decision-cli.dev/ns#Session>",
        vga_iri.as_str()
    );
    assert!(
        dump_body.contains(&session_class_line),
        "expected dec:Session typing quad for {}; not in dump",
        vga_iri.as_str()
    );
    // dec:roleId literal lands as "verify-graph-author".
    assert!(
        dump_body.contains(&format!(
            "<{}> <https://decision-cli.dev/ns#roleId> \"verify-graph-author\"",
            vga_iri.as_str()
        )),
        "expected verify-graph-author role literal"
    );
    // Status starts as completed.
    assert!(dump_body.contains(&format!(
        "<{}> <https://decision-cli.dev/ns#status> \"completed\"",
        vga_iri.as_str()
    )));

    // ----- Scenario B: implementer dispatch IRI with `Failed` status -----
    let imp_iri = NamedNode::new_unchecked(
        "https://decision-cli.dev/ns/activity/implement/disp-T193b",
    );
    materialize(
        wd.path(),
        &imp_iri,
        "implementer",
        "FT-T193b",
        "2026-05-27T11:00:00Z",
        "2026-05-27T11:00:45Z",
        DispatchStatus::Failed,
    )
    .expect("implementer materialize");

    let dump_body = fs::read_to_string(wd.path().join(".dec/store/orchestration.nq"))
        .expect("read dump");
    assert!(
        dump_body.contains(&format!(
            "<{}> <https://decision-cli.dev/ns#roleId> \"implementer\"",
            imp_iri.as_str()
        )),
        "expected implementer role literal"
    );
    assert!(
        dump_body.contains(&format!(
            "<{}> <https://decision-cli.dev/ns#status> \"failed\"",
            imp_iri.as_str()
        )),
        "expected dec:status \"failed\" for the failed dispatch"
    );
}
