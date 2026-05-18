//! TC-015 — Bootstrap session reachable from `ValueStream` via PROV-O.
//! Validates: FT-008, FT-009 · ADR-004.
//! Spec: .product/tests/TC-015-bootstrap-session-reachable-from-valuestream-via-p.md

#[test]
fn bootstrap_session_reachable_from_valuestream_via_provo() {
    panic!(
        "TC-015 not yet implemented.\n\n\
         Required: run `dec init` (FT-008) against a fresh store, then \
         assert <dec:session/init-001> a dec:Session and that the \
         ValueStream prov:wasGeneratedBy <dec:session/init-001>."
    );
}
