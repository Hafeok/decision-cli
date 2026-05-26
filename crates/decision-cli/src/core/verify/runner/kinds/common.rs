//! Shared helpers for the kind handlers.

use crate::core::ontology::verification_result::StepOutcome;
use crate::core::vocab::EXCERPT_CAP_BYTES;

/// Current UTC instant as an ISO 8601 string.
pub(super) fn iso_now() -> String {
    chrono::Utc::now()
        .format("%Y-%m-%dT%H:%M:%S%.3fZ")
        .to_string()
}

/// Cap a byte buffer to the per-FT-097 excerpt limit; lossy UTF-8 decode.
pub(super) fn cap_excerpt(bytes: &[u8]) -> String {
    let take = bytes.len().min(EXCERPT_CAP_BYTES);
    String::from_utf8_lossy(&bytes[..take]).into_owned()
}

/// True if the outcome represents a successful step.
#[allow(dead_code)]
pub(super) const fn is_pass(o: StepOutcome) -> bool {
    matches!(o, StepOutcome::Pass)
}
