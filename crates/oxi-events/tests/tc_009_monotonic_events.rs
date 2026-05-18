//! TC-009 — GraphWriter events have monotonic, contiguous sequence numbers.
//! Validates: FT-001, FT-002, FT-003, FT-005 · ADR-002.
//! Spec: .product/tests/TC-009-graphwriter-events-have-monotonic-sequence-numbers.md

#[test]
fn graphwriter_events_have_monotonic_sequence_numbers() {
    panic!(
        "TC-009 not yet implemented.\n\n\
         Required: open a GraphWriter (FT-001), register a subscription \
         (FT-002), perform N>=5 mutations, then SPARQL the events graph \
         and assert strictly-increasing contiguous seq."
    );
}
