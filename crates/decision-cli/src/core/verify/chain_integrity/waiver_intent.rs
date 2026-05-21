//! Waiver intent — the structured input the caller passes to the
//! chain-integrity gate when they want to override an uncovered
//! feature dispatch (FT-047 §Behaviour step 5, §Error handling).
//!
//! The intent is validated **before** any waiver artifact is minted so
//! the gate can return `Error::InvalidArgument` with exit-code 2 for a
//! malformed reason — no on-disk side-effect happens in that case.

use thiserror::Error;

use crate::core::vocab::WAIVER_REASON_MIN_LEN;

/// Caller-supplied intent to override the chain-integrity gate.
///
/// Mirrors the `--waive-coverage <reason>` CLI flag and the
/// `accept_waiver: { reason }` MCP field per FT-047 §Outputs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WaiverIntent {
    /// Free-form prose. Must be ≥ 16 non-whitespace characters.
    pub reason: String,
}

impl WaiverIntent {
    /// Construct an intent from a raw reason string.
    #[must_use]
    pub fn new(reason: impl Into<String>) -> Self {
        Self {
            reason: reason.into(),
        }
    }
}

/// Structured failure for a malformed waiver reason.
///
/// FT-047 §Error handling: short or whitespace-only reason → exit 2.
#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum WaiverReasonError {
    /// The reason has fewer than [`WAIVER_REASON_MIN_LEN`] non-whitespace
    /// characters (the whitespace-only case maps here with `non_ws = 0`).
    #[error(
        "Error::InvalidArgument: field 'waiver.reason' — \
         must be at least {WAIVER_REASON_MIN_LEN} non-whitespace characters \
         (got {non_ws}; whitespace-only input is rejected)"
    )]
    TooShort {
        /// Count of non-whitespace characters in the input.
        non_ws: usize,
    },
}

/// Validate a waiver reason against the ADR-031 / FT-047 minimum-length
/// rule. Returns the cleaned-up reason on success.
///
/// The rule is *count of non-whitespace characters ≥ 16*; this catches
/// both short strings and whitespace-only strings in one check, exactly
/// as TC-075 requires.
pub fn validate_waiver_reason(reason: &str) -> Result<String, WaiverReasonError> {
    let non_ws = reason.chars().filter(|c| !c.is_whitespace()).count();
    if non_ws < WAIVER_REASON_MIN_LEN {
        return Err(WaiverReasonError::TooShort { non_ws });
    }
    Ok(reason.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn long_enough_reason_passes() {
        let r = "Doc-only feature; verification is review-based per ADR-NNN";
        assert!(validate_waiver_reason(r).is_ok());
    }

    #[test]
    fn short_reason_is_rejected() {
        let r = "too short";
        let err = validate_waiver_reason(r).expect_err("must reject");
        match err {
            WaiverReasonError::TooShort { non_ws } => {
                assert!(non_ws < WAIVER_REASON_MIN_LEN);
            }
        }
    }

    #[test]
    fn whitespace_only_reason_is_rejected_with_zero_count() {
        let r = "                                ";
        let err = validate_waiver_reason(r).expect_err("must reject");
        match err {
            WaiverReasonError::TooShort { non_ws } => {
                assert_eq!(non_ws, 0);
            }
        }
    }

    #[test]
    fn error_message_names_the_field_and_the_minimum() {
        let err = validate_waiver_reason("hi").expect_err("rejects");
        let s = err.to_string();
        assert!(s.contains("Error::InvalidArgument"));
        assert!(s.contains("waiver.reason"));
        assert!(s.contains("16"));
        assert!(s.contains("whitespace-only"));
    }

    #[test]
    fn exactly_min_length_passes() {
        let r = "exactlysixteench"; // 16 chars, no whitespace
        assert_eq!(r.len(), 16);
        assert!(validate_waiver_reason(r).is_ok());
    }
}
