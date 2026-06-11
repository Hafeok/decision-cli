//! WorkerCurator verdict input shape (FT-092).
//!
//! The verdict carries the Curator's decision over a CuratorBundle —
//! one of two cases per FT-092 §Scope:
//!
//! - **Admit**: the Curator authorises minting a `dec:WorkerImage` and
//!   attaching a `dec:ConformanceAudit` of class `manual-review`. The
//!   rationale becomes the audit's notes field.
//!
//! - **Reject**: the Curator refuses admission. `rationale` becomes the
//!   Feedback's `recommendation` field, `disqualification_evidence`
//!   becomes the `evidence` field. No WorkerImage is minted.

/// Curator's decision over a [`super::CuratorBundle`].
///
/// The two variants exhaust the FT-092 §Scope output cases.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CuratorVerdict {
    /// Admit the Submission — mint a `dec:WorkerImage` with
    /// `eligibility_status=qualified` and attach a `dec:ConformanceAudit`
    /// of class `manual-review`.
    Admit {
        /// Operator-facing rationale captured on the ConformanceAudit's
        /// `dec:audit_notes` field. Non-empty.
        rationale: String,
    },
    /// Reject the Submission — emit a `dec:Feedback` artifact pointing
    /// at the Submission.
    Reject {
        /// Operator-facing rationale captured on the Feedback's
        /// `dec:recommendation` field. Non-empty.
        rationale: String,
        /// Specific evidence pointing at what disqualified the
        /// Submission — captured on the Feedback's `dec:evidence`
        /// field. Non-empty per FT-026 SHACL.
        disqualification_evidence: String,
    },
}

impl CuratorVerdict {
    /// Discriminator helper for downstream queries that only need to
    /// know whether the Curator admitted or rejected.
    #[must_use]
    pub const fn is_admit(&self) -> bool {
        matches!(self, Self::Admit { .. })
    }

    /// Convenience helper mirroring [`Self::is_admit`].
    #[must_use]
    pub const fn is_reject(&self) -> bool {
        matches!(self, Self::Reject { .. })
    }
}
