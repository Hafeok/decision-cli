//! Shared `DefectFeedbackRecord` shape — the structured record that the
//! verify-graph-author bundle (FT-107) and the code-writer dispatch
//! payload (FT-108) both ship to their respective workers.
//!
//! Lives in `core::feedback` rather than under either feature so the two
//! consumers stay slice-independent per the slice-level SDP rule in
//! `CLAUDE.md` ("Features depend on `core/`; never on other features").

use serde::{Deserialize, Serialize};

/// One defect-feedback entry surfaced to a worker bundle.
///
/// Both the verify-graph-author bundle and the implementer dispatch
/// payload carry zero-or-more of these. The `class` is always
/// `"defect"` for records either loader returns; the field is kept on
/// the wire shape so the worker can pattern-match if other classes get
/// surfaced later.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DefectFeedbackRecord {
    /// Stable feedback IRI (`urn:dec:feedback:<uuid>`).
    pub feedback_iri: String,
    /// Always `"defect"` for records the loaders return.
    pub class: String,
    /// `"error"` | `"warning"` | `"info"`.
    pub severity: String,
    /// Free-form evidence excerpt the runner wrote at emission time.
    pub evidence: String,
    /// TC IRI the feedback's `dec:sourceArtifact` points at.
    pub source_tc: String,
    /// Short id of the graph whose failing step covers `source_tc`
    /// (`VG-007`). Empty when the loader cannot uniquely resolve the
    /// graph (the implementer loader leaves this empty since it joins
    /// by TC, not by (feature, env, graph)).
    pub graph_id: String,
}
