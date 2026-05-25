//! TC-150 — pipeline-worker SDK: Build-time codegen of typed Bundle and
//! Artifact surfaces from SHACL (FT-085 / ADR-048).
//!
//! Exit criterion for FT-085. Asserts the codegen tool shipped at
//! `workers/pipeline-worker-sdk/tools/codegen/` produces:
//!
//! 1. A generated tree under
//!    `workers/pipeline-worker-sdk/src/pipeline_worker_sdk/{bundle,artifact,schemas}/_generated/`.
//! 2. One module per per-type SHACL shape (`PER_TYPE_SHAPE_FILES`) in
//!    each of the three packages. The set of generated module
//!    basenames matches the snake-cased local names of the target
//!    classes published in `PER_TYPE_SHAPE_IRIS`.
//! 3. Every generated module carries the "GENERATED FILE — DO NOT EDIT"
//!    banner declared by the emitters. This is the audit trail that
//!    keeps reviewers from hand-editing the output.
//! 4. Each `_generated/__init__.py` re-exports the typed surface for
//!    the package (e.g. `FeatureBuilder`, `FeatureAccessor`,
//!    `FeatureSchema`) — proving that the codegen wires the public
//!    surface, not just the per-module files.
//! 5. Byte-stability: when Python and the codegen tool are available,
//!    re-running `python -m tools.codegen --check` against the
//!    checked-in tree exits 0 (no drift). Skips with a log line if
//!    Python or the SDK venv is not present — Rust-side structural
//!    assertions remain authoritative.
//!
//! Structural assertions (1)–(4) are *always* enforced. The
//! byte-stability assertion (5) is a defense-in-depth check that
//! verifies the actual codegen output matches what's been checked in —
//! it requires Python + the SDK's `.venv`, both of which are present in
//! the development environment but may legitimately be missing in CI
//! configurations that do not pre-install the worker SDK.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const GENERATED_BANNER: &str = "GENERATED FILE — DO NOT EDIT BY HAND";

/// Per-type SHACL shape file → (snake_case module basename, target class
/// local name). One row per file in
/// [`decision_cli::core::ontology::PER_TYPE_SHAPE_FILES`]. The class
/// local name is the suffix the codegen tool reads off the SHACL
/// `sh:targetClass` IRI — abbreviations like `ADR` preserve their
/// upstream casing, which is why we keep an explicit mapping here
/// rather than mechanically deriving it from the module basename.
const EXPECTED_MODULES: &[(&str, &str, &str)] = &[
    ("acknowledgement.ttl", "acknowledgement", "Acknowledgement"),
    ("adr.ttl", "adr", "ADR"),
    ("brief.ttl", "brief", "Brief"),
    ("conformance-audit.ttl", "conformance_audit", "ConformanceAudit"),
    ("dependency.ttl", "dependency", "Dependency"),
    ("discovery-finding.ttl", "discovery_finding", "DiscoveryFinding"),
    ("dispatch.ttl", "dispatch", "Dispatch"),
    ("feature.ttl", "feature", "Feature"),
    ("feedback.ttl", "feedback", "Feedback"),
    ("model.ttl", "model", "Model"),
    ("policy.ttl", "policy", "Policy"),
    ("query-template.ttl", "query_template", "QueryTemplate"),
    ("question.ttl", "question", "Question"),
    ("subscription.ttl", "subscription", "Subscription"),
    ("tc.ttl", "tc", "TC"),
    (
        "worker-image-submission.ttl",
        "worker_image_submission",
        "WorkerImageSubmission",
    ),
    ("worker-image.ttl", "worker_image", "WorkerImage"),
];

const GENERATED_PACKAGES: &[(&str, &str)] = &[
    (
        "workers/pipeline-worker-sdk/src/pipeline_worker_sdk/bundle/_generated",
        "Accessor",
    ),
    (
        "workers/pipeline-worker-sdk/src/pipeline_worker_sdk/artifact/_generated",
        "Builder",
    ),
    (
        "workers/pipeline-worker-sdk/src/pipeline_worker_sdk/schemas/_generated",
        "Schema",
    ),
];

#[test]
fn tc_150_pipeline_worker_sdk_build_time_codegen_of_typed_bu() {
    let repo_root = repo_root();

    // ---- (1) Generated directories exist ------------------------------
    for (rel, _) in GENERATED_PACKAGES {
        let dir = repo_root.join(rel);
        assert!(
            dir.is_dir(),
            "FT-085: expected generated directory at {} — \
             run `cd workers/pipeline-worker-sdk && uv run codegen` to create it",
            dir.display()
        );
    }

    // ---- (2) Per-type modules exist in every package ------------------
    for (pkg_rel, suffix) in GENERATED_PACKAGES {
        let dir = repo_root.join(pkg_rel);
        let on_disk = collect_py_basenames(&dir);
        let expected: BTreeSet<String> = EXPECTED_MODULES
            .iter()
            .map(|(_, mod_name, _)| (*mod_name).to_string())
            .chain(std::iter::once("__init__".to_string()))
            .collect();
        assert_eq!(
            on_disk,
            expected,
            "FT-085: package `{pkg_rel}` (suffix `{suffix}`) has module \
             set\n  on-disk: {:?}\n  expected: {:?}\n— rerun `uv run codegen`",
            on_disk.iter().collect::<Vec<_>>(),
            expected.iter().collect::<Vec<_>>()
        );
    }

    // ---- (3) Generated banner on every per-type module ---------------
    for (pkg_rel, _) in GENERATED_PACKAGES {
        let dir = repo_root.join(pkg_rel);
        for (_, mod_name, _) in EXPECTED_MODULES {
            let path = dir.join(format!("{mod_name}.py"));
            let body =
                fs::read_to_string(&path).unwrap_or_else(|e| {
                    panic!("FT-085: cannot read {}: {e}", path.display())
                });
            assert!(
                body.contains(GENERATED_BANNER),
                "FT-085: {} must contain banner '{GENERATED_BANNER}' — \
                 the emitter ships it on every file so reviewers see \
                 'do not edit by hand' immediately",
                path.display()
            );
            assert!(
                body.contains("Source SHACL shape: workers/_shared/shapes/"),
                "FT-085: {} must record its source shape file in the header",
                path.display()
            );
        }
    }

    // ---- (4) __init__.py re-exports every typed class ---------------
    for (pkg_rel, suffix) in GENERATED_PACKAGES {
        let init_path = repo_root.join(pkg_rel).join("__init__.py");
        let body = fs::read_to_string(&init_path)
            .unwrap_or_else(|e| panic!("read {}: {e}", init_path.display()));
        assert!(
            body.contains(GENERATED_BANNER),
            "FT-085: {} must carry the generated banner",
            init_path.display()
        );
        // Expect every type's class name to appear (e.g. FeatureBuilder).
        for (_, mod_name, class_local) in EXPECTED_MODULES {
            let class_name = format!("{class_local}{suffix}");
            assert!(
                body.contains(&class_name),
                "FT-085: {} must re-export {class_name} (the emitter writes \
                 `from .{mod_name} import {class_name}` for every shape)",
                init_path.display()
            );
        }
    }

    // ---- (5) Byte-stability via the codegen tool (best-effort) ------
    byte_stability_check_via_python(&repo_root);
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn repo_root() -> PathBuf {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest
        .parent()
        .and_then(|p| p.parent())
        .expect("repo root from CARGO_MANIFEST_DIR")
        .to_path_buf()
}

fn collect_py_basenames(dir: &Path) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(err) => panic!("read_dir {}: {err}", dir.display()),
    };
    for entry in entries {
        let entry = entry.expect("dirent");
        if !entry.file_type().map(|t| t.is_file()).unwrap_or(false) {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        if let Some(stem) = name.strip_suffix(".py") {
            out.insert(stem.to_string());
        }
    }
    out
}

/// Best-effort byte-stability check: re-runs the codegen tool with
/// `--check`. Exit 0 = generated tree matches; exit 1 = drift; exit 2 =
/// shapes-dir missing. We assert exit 0. If Python or the SDK venv is
/// unavailable, log and skip — the Rust-side structural assertions are
/// authoritative for the TC's invariants.
fn byte_stability_check_via_python(repo_root: &Path) {
    let sdk = repo_root.join("workers/pipeline-worker-sdk");
    if !sdk.is_dir() {
        eprintln!("[TC-150] SDK directory missing; skipping byte-stability check");
        return;
    }

    // Prefer the SDK's own venv interpreter when present (matches the
    // pyproject's resolved pyoxigraph/pydantic exactly).
    let venv_python = sdk.join(".venv/bin/python");
    let python = if venv_python.exists() {
        venv_python.to_string_lossy().to_string()
    } else {
        std::env::var("PYTHON").unwrap_or_else(|_| "python3".to_string())
    };

    // Probe the interpreter quickly.
    if Command::new(&python).arg("--version").output().is_err() {
        eprintln!(
            "[TC-150] python interpreter `{python}` unavailable; \
             skipping byte-stability check"
        );
        return;
    }

    // Quick import test: pyoxigraph must be available for the codegen.
    let probe_pyox = Command::new(&python)
        .args(["-c", "import pyoxigraph"])
        .current_dir(&sdk)
        .output();
    match probe_pyox {
        Ok(o) if o.status.success() => {}
        _ => {
            eprintln!(
                "[TC-150] pyoxigraph not importable in `{python}`; \
                 skipping byte-stability check (the structural \
                 assertions above remain authoritative)"
            );
            return;
        }
    }

    let out = Command::new(&python)
        .args(["-m", "tools.codegen", "--check", "--quiet"])
        .current_dir(&sdk)
        .output();
    let out = match out {
        Ok(o) => o,
        Err(err) => {
            eprintln!(
                "[TC-150] could not invoke codegen tool: {err}; skipping \
                 byte-stability check"
            );
            return;
        }
    };
    if !out.status.success() {
        let stdout = String::from_utf8_lossy(&out.stdout);
        let stderr = String::from_utf8_lossy(&out.stderr);
        panic!(
            "FT-085: codegen --check reported drift — the checked-in \
             generated tree does not match what the shapes currently \
             produce. Run `cd workers/pipeline-worker-sdk && uv run \
             codegen` and commit the result.\n\nstdout:\n{stdout}\n\nstderr:\n{stderr}"
        );
    }
}
