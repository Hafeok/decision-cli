//! Declaration of a TaskType - a named cluster of Cells with a coherence audit.

use crate::core::task_type::cell_decl::CellDecl;
use crate::core::task_type::coherence_audit::CoherenceAuditSpec;
use std::path::PathBuf;

/// A TaskType declares a named cluster of Cells with a coherence audit.
#[derive(Debug, Clone)]
pub struct TaskTypeDecl {
    /// The unique name of this TaskType.
    pub name: String,

    /// The ordered cluster of Cells that compose this TaskType.
    pub cells: Vec<CellDecl>,

    /// Specification for the coherence audit that validates the cluster.
    pub coherence_audit: CoherenceAuditSpec,
}