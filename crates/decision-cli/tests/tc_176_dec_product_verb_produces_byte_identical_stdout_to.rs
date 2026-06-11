//! TC-176 — `dec product <verb>` produces byte-identical stdout to `product <verb>`.
//!
//! Validates: FT-105 · ADR-067.
//! Spec: `.product/tests/TC-176-dec-product-verb-produces-byte-identical-stdout-to.md`
//!
//! Both surfaces route through `product_cli::dispatch` — the in-workspace
//! product-cli crate is the single source of truth. This test exercises
//! the parity at the binary level: it invokes the `dec` binary built by
//! cargo for the integration test, and the `product` shim binary that
//! sits next to it in the same target directory, comparing observable
//! stdout / stderr for the verbs named in the FT-105 §Parity set.

use std::path::{Path, PathBuf};
use std::process::Command;

/// Locate the `product` shim binary by walking from `CARGO_BIN_EXE_dec`'s
/// directory. cargo only auto-sets `CARGO_BIN_EXE_<name>` for binaries
/// in the *current* package (decision-cli), so the shim's path is
/// computed by replacing the filename.
fn product_shim_path() -> PathBuf {
    let dec: PathBuf = env!("CARGO_BIN_EXE_dec").into();
    let dir = dec.parent().expect("dec binary has a parent directory");
    // Honour Windows .exe suffix if cargo grew one for the dec binary.
    if let Some(ext) = dec.extension() {
        let mut name = std::ffi::OsString::from("product");
        name.push(".");
        name.push(ext);
        dir.join(name)
    } else {
        dir.join("product")
    }
}

fn dec_path() -> &'static Path {
    Path::new(env!("CARGO_BIN_EXE_dec"))
}

/// Build product if it isn't there. cargo test normally builds the
/// dec binary (because integration tests depend on it), but the
/// product shim lives in a different workspace package and is not
/// transitively built unless something pulls it in. The test does
/// the build itself so a fresh `cargo test` cycle works.
fn ensure_product_shim_built() {
    let shim = product_shim_path();
    if shim.exists() {
        return;
    }
    let status = Command::new(env!("CARGO"))
        .args(["build", "-p", "product-shim", "--quiet"])
        .status()
        .expect("invoke cargo build for product-shim");
    assert!(status.success(), "cargo build product-shim failed");
    assert!(
        shim.exists(),
        "product shim binary missing after cargo build: {}",
        shim.display()
    );
}

fn run(cmd: &Path, args: &[&str]) -> (i32, String, String) {
    let output = Command::new(cmd)
        .args(args)
        .output()
        .unwrap_or_else(|e| panic!("failed to execute {}: {e}", cmd.display()));
    let code = output.status.code().unwrap_or(-1);
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    (code, stdout, stderr)
}

const DEPRECATION_WARNING: &str = "warning: 'product' is deprecated; prefer 'dec product <verb>'";

/// Strip the deprecation-shim warning line from a stderr stream so the
/// parity comparison ignores it (Scenario D).
fn strip_deprecation_warning(stderr: &str) -> String {
    stderr
        .lines()
        .filter(|line| !line.contains(DEPRECATION_WARNING))
        .collect::<Vec<_>>()
        .join("\n")
}

fn assert_parity(verb_label: &str, dec_args: &[&str], product_args: &[&str]) {
    let (dec_rc, dec_out, dec_err) = run(dec_path(), dec_args);
    let (prod_rc, prod_out, prod_err) = run(&product_shim_path(), product_args);

    assert_eq!(
        dec_rc, prod_rc,
        "exit-code parity mismatch for {verb_label}: dec={dec_rc} product={prod_rc}\n\
         dec stdout: {dec_out}\nproduct stdout: {prod_out}"
    );
    assert_eq!(
        dec_out, prod_out,
        "stdout parity mismatch for {verb_label}\n--- dec ---\n{dec_out}\n--- product ---\n{prod_out}"
    );
    // Scenario D: dec stderr must NOT contain the deprecation warning.
    assert!(
        !dec_err.contains(DEPRECATION_WARNING),
        "dec stderr unexpectedly contains the deprecation warning: {dec_err}"
    );
    // Scenario D: product stderr DOES contain the deprecation warning.
    assert!(
        prod_err.contains(DEPRECATION_WARNING),
        "product stderr missing the deprecation warning: {prod_err}"
    );
    // Scenario D: after stripping the warning, stderrs are equal.
    let prod_err_stripped = strip_deprecation_warning(&prod_err);
    assert_eq!(
        dec_err.trim_end(),
        prod_err_stripped.trim_end(),
        "stderr parity (post-strip) mismatch for {verb_label}"
    );
}

#[test]
fn tc_176_dec_product_verb_produces_byte_identical_stdout_to() {
    ensure_product_shim_built();

    // Parity set — FT-105 §Parity table. The standalone-product invocation
    // omits the leading `dec product` words, the dec form keeps them.
    let cases: &[(&str, &[&str], &[&str])] = &[
        (
            "feature show FT-001",
            &["product", "feature", "show", "FT-001"],
            &["feature", "show", "FT-001"],
        ),
        (
            "feature list",
            &["product", "feature", "list"],
            &["feature", "list"],
        ),
        (
            "feature next",
            &["product", "feature", "next"],
            &["feature", "next"],
        ),
        (
            "adr show ADR-001",
            &["product", "adr", "show", "ADR-001"],
            &["adr", "show", "ADR-001"],
        ),
        ("adr list", &["product", "adr", "list"], &["adr", "list"]),
        (
            "context FT-001",
            &["product", "context", "FT-001"],
            &["context", "FT-001"],
        ),
        (
            "preflight FT-001",
            &["product", "preflight", "FT-001"],
            &["preflight", "FT-001"],
        ),
        (
            "graph check",
            &["product", "graph", "check"],
            &["graph", "check"],
        ),
    ];

    for (label, dec_args, product_args) in cases {
        assert_parity(label, dec_args, product_args);
    }
}
