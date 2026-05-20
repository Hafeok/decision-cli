use std::path::Path;

use super::{build_commit_message, summarise, FinalizeInput};

#[test]
fn commit_message_includes_iris_and_short_hash() {
    let input = FinalizeInput {
        repo_root: Path::new("/tmp"),
        product_root: Path::new("/tmp"),
        feature_id: "FT-099",
        session_iri: "https://decision-cli.dev/ns/session/abc",
        dispatch_iri: "https://decision-cli.dev/ns/dispatch/def",
        code_change_iri: "https://decision-cli.dev/ns/code-change/ghi",
        bundle_hash: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
        worker_summary: "Land the thing\n\nMore detail follows.",
    };
    let msg = build_commit_message(&input);
    assert!(msg.starts_with("[FT-099] Land the thing\n\n"), "subject");
    assert!(msg.contains("Session:     https://decision-cli.dev/ns/session/abc"));
    assert!(msg.contains("Dispatch:    https://decision-cli.dev/ns/dispatch/def"));
    assert!(msg.contains("CodeChange:  https://decision-cli.dev/ns/code-change/ghi"));
    assert!(msg.contains("Bundle:      sha256:0123456789abcdef"));
}

#[test]
fn commit_message_drops_codechange_line_when_empty() {
    let input = FinalizeInput {
        repo_root: Path::new("/tmp"),
        product_root: Path::new("/tmp"),
        feature_id: "FT-099",
        session_iri: "s",
        dispatch_iri: "d",
        code_change_iri: "",
        bundle_hash: "deadbeefdeadbeef",
        worker_summary: "x",
    };
    let msg = build_commit_message(&input);
    assert!(!msg.contains("CodeChange:"), "{msg}");
}

#[test]
fn summary_truncates_long_first_line() {
    let s = "a".repeat(120);
    let out = summarise(&s);
    assert_eq!(out.chars().count(), 72);
    assert!(out.ends_with('…'));
}

#[test]
fn summary_picks_first_nonblank_line() {
    let out = summarise("\n\n   actual line   \nnext line");
    assert_eq!(out, "actual line");
}
