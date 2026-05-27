//! TC-194 — `loop_inspect::resolver` short-id rules:
//!
//!   * `activity/verify-graph-run/VG-NNN/...` → `VG-NNN`
//!   * `activity/verify-graph-generate/VG-NNN` → `verify-graph-author@VG-NNN`
//!   * `activity/implement/<dispatch-id>` → `implement@<first-12>`
//!   * `graph/VG-NNN[-suffix]` → `VG-NNN[-suffix]`
//!   * `code-change/CC-NNN` → `CC-NNN`
//!   * `urn:dec:feedback:<uuid>` → `fb:<first8>`
//!   * `tc/TC-NNN` → `TC-NNN`
//!   * Unknown → raw IRI.
//!
//! Validates: FT-109.

use decision_cli::loop_inspect::resolver::{
    short_for_artifact, short_for_feedback, short_for_session, short_for_tc,
};

#[test]
fn tc_194_loop_show_short_id_resolution() {
    // Session IRIs.
    assert_eq!(
        short_for_session("https://decision-cli.dev/ns/activity/verify-graph-run/VG-007/ts-1234"),
        "VG-007"
    );
    assert_eq!(
        short_for_session("https://decision-cli.dev/ns/activity/verify-graph-generate/VG-098"),
        "verify-graph-author@VG-098"
    );
    assert!(short_for_session("https://decision-cli.dev/ns/activity/implement/disp-abcdef0123456789")
        .starts_with("implement@"));

    // Artifact IRIs.
    assert_eq!(
        short_for_artifact("https://decision-cli.dev/ns/graph/VG-NEW-1"),
        "VG-NEW-1"
    );
    assert_eq!(
        short_for_artifact("https://decision-cli.dev/ns/code-change/CC-FIX-2"),
        "CC-FIX-2"
    );

    // Feedback IRIs.
    assert_eq!(
        short_for_feedback("urn:dec:feedback:abcd1234-abcd-abcd-abcd-abcdabcdabcd"),
        "fb:abcd1234"
    );

    // TC IRIs.
    assert_eq!(
        short_for_tc("https://decision-cli.dev/ns/tc/TC-041"),
        "TC-041"
    );

    // Unknown — falls back to the raw IRI.
    let raw = "http://example.org/something-else";
    assert_eq!(short_for_session(raw), raw);
    assert_eq!(short_for_artifact(raw), raw);
    assert_eq!(short_for_feedback(raw), raw);
    assert_eq!(short_for_tc(raw), raw);
}
